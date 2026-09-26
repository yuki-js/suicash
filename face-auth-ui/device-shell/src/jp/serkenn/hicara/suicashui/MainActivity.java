package jp.serkenn.hicara.suicashui;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.util.Base64;
import android.util.Log;
import android.view.TextureView;
import android.view.ViewGroup;
import android.webkit.JavascriptInterface;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;

import java.io.ByteArrayOutputStream;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

import org.json.JSONObject;

import jp.co.nextware.frcardreader.safr.EARDetectedFace;

/**
 * face-auth-ui (React) を全画面表示する WebView シェル。
 *
 * - 顔認証エンジン(SAFR eSDK)を同プロセスで初期化し JS ブリッジで公開
 * - カメラはネイティブ(Camera2 → TextureView)が所有し、プレビュー JPEG を
 *   低 fps で JS に push(window.__safrFrame)。WebView の getUserMedia は
 *   この端末の HAL とかみ合わないため使わない
 * - probe / register / match はネイティブが現在フレームを直接取るので、
 *   JS からの画像転送は無い
 *
 * ブリッジ:
 *   JS  → Java : SafrNative.request(id, method, payload)
 *                (status / probe / register / match / clearStore /
 *                 cameraStart / cameraStop)
 *   Java → JS  : window.__safrResolve(id, resultJson)
 *   Java → JS  : window.__safrFrame(dataUrl)   … プレビューフレーム push
 *
 * URL は intent data で差し替え可能(singleTask + onNewIntent なので
 * 母艦 CLI から am start -d <url> で画面・モードを切り替えられる)。
 */
public class MainActivity extends Activity {

    private static final String TAG = "SuiCashUI";
    private static final String DEFAULT_URL = "http://localhost:5173/?debug=1";
    private static final int PREVIEW_INTERVAL_MS = 250; // 約4fps
    private static final int PREVIEW_WIDTH = 320;
    private static final int PROBE_WIDTH = 640;
    private static final int CAPTURE_WIDTH = 960;

    private WebView web;
    private CameraHost cameraHost;
    private final SafrEngine engine = new SafrEngine();
    /** 上部 RGB LED(SmartETK GPIO) */
    private final LedController led = new LedController();
    /** エンジン呼び出しは単一スレッドで直列化する */
    private final ExecutorService engineExec = Executors.newSingleThreadExecutor();
    /** LED は SAFR と別スレッド(GPIO の遅延が SAFR を止めないように) */
    private final ExecutorService ledExec = Executors.newSingleThreadExecutor();
    /** プレビュー push はエンジンと独立に回す */
    private final ExecutorService previewExec = Executors.newSingleThreadExecutor();
    private volatile boolean previewOn = false;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        if (checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] { Manifest.permission.CAMERA }, 1);
        }

        // エンジン初期化(初回はモデル展開込みで数十秒)
        engineExec.execute(() -> {
            boolean ok = engine.init(getApplicationContext());
            Log.i(TAG, "SafrEngine init " + (ok ? "OK" : "FAILED"));
        });
        // LED 初期化(GPIO サービスが無ければ無効で続行)
        ledExec.execute(led::init);

        FrameLayout root = new FrameLayout(this);

        TextureView texture = new TextureView(this);
        root.addView(texture, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        cameraHost = new CameraHost(this, texture);

        web = new WebView(this);
        web.setBackgroundColor(Color.parseColor("#060E1A"));
        WebView.setWebContentsDebuggingEnabled(true);

        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);

        web.addJavascriptInterface(new SafrBridge(), "SafrNative");
        web.addJavascriptInterface(new LedBridge(), "LedNative");
        web.setWebViewClient(new WebViewClient());
        root.addView(web, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        setContentView(root);
        load(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        load(intent); // 母艦 CLI からの再 start でモード切替
    }

    private void load(Intent intent) {
        Uri data = intent != null ? intent.getData() : null;
        String url = data != null ? data.toString() : DEFAULT_URL;
        Log.i(TAG, "load " + url);
        stopPreview();
        web.loadUrl(url);
    }

    @Override
    protected void onPause() {
        stopPreview();
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        stopPreview();
        led.release();
        engineExec.shutdown();
        ledExec.shutdown();
        previewExec.shutdown();
        if (web != null) {
            web.destroy();
        }
        super.onDestroy();
    }

    /** WebView から LED を制御する JS ブリッジ: window.LedNative.set("blue_blink"|"green"|"red"|"off") */
    private class LedBridge {
        @android.webkit.JavascriptInterface
        public void set(final String mode) {
            ledExec.execute(() -> led.setByName(mode));
        }
    }

    // ---------------------------------------------------------- プレビュー push

    private void startPreview() {
        if (previewOn) {
            return;
        }
        previewOn = true;
        cameraHost.start();
        previewExec.execute(() -> {
            while (previewOn) {
                long t0 = System.currentTimeMillis();
                try {
                    Bitmap bmp = cameraHost.grabUpright(PREVIEW_WIDTH);
                    if (bmp != null) {
                        ByteArrayOutputStream bos = new ByteArrayOutputStream();
                        bmp.compress(Bitmap.CompressFormat.JPEG, 60, bos);
                        String dataUrl = "data:image/jpeg;base64,"
                                + Base64.encodeToString(bos.toByteArray(), Base64.NO_WRAP);
                        runOnUiThread(() -> web.evaluateJavascript(
                                "window.__safrFrame && window.__safrFrame("
                                        + JSONObject.quote(dataUrl) + ")",
                                null));
                    }
                } catch (Throwable t) {
                    Log.w(TAG, "preview push failed", t);
                }
                long rest = PREVIEW_INTERVAL_MS - (System.currentTimeMillis() - t0);
                if (rest > 0) {
                    try {
                        Thread.sleep(rest);
                    } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                        return;
                    }
                }
            }
        });
    }

    private void stopPreview() {
        previewOn = false;
        if (cameraHost != null) {
            cameraHost.stop();
        }
    }

    // ------------------------------------------------------------- JS ブリッジ

    private class SafrBridge {

        @JavascriptInterface
        public void request(final String id, final String method, final String payload) {
            // カメラ制御はエンジン処理と独立に即時実行する
            if ("cameraStart".equals(method) || "cameraStop".equals(method)) {
                if ("cameraStart".equals(method)) {
                    startPreview();
                } else {
                    stopPreview();
                }
                resolve(id, "{}");
                return;
            }
            engineExec.execute(() -> {
                String json;
                try {
                    json = handle(method);
                } catch (Throwable t) {
                    Log.e(TAG, "bridge " + method + " failed", t);
                    json = "{}";
                }
                resolve(id, json);
            });
        }

        private void resolve(String id, String resultJson) {
            runOnUiThread(() -> web.evaluateJavascript(
                    "window.__safrResolve && window.__safrResolve("
                            + JSONObject.quote(id) + ","
                            + JSONObject.quote(resultJson) + ")",
                    null));
        }

        private String handle(String method) throws Exception {
            switch (method) {
                case "status": {
                    JSONObject o = new JSONObject();
                    o.put("ready", engine.isInitialized());
                    o.put("registered", engine.hasPerson());
                    o.put("camera", cameraHost.isRunning());
                    return o.toString();
                }
                case "probe": {
                    JSONObject o = new JSONObject();
                    Bitmap bmp = cameraHost.grabUpright(PROBE_WIDTH);
                    if (bmp == null) {
                        o.put("found", false);
                        return o.toString();
                    }
                    SafrEngine.Probe p = engine.probe(bmp);
                    o.put("found", p.found);
                    if (p.found) {
                        EARDetectedFace f = p.face;
                        o.put("cpq", f.centerPoseQuality);
                        o.put("contrast", f.contrastQuality);
                        o.put("sharpness", f.sharpnessQuality);
                        o.put("mask", f.mask);
                        o.put("x", f.boundsX);
                        o.put("y", f.boundsY);
                        o.put("w", f.boundsWidth);
                        o.put("h", f.boundsHeight);
                    }
                    return o.toString();
                }
                case "register": {
                    Bitmap bmp = cameraHost.grabUpright(CAPTURE_WIDTH);
                    SafrEngine.Result r = (bmp == null) ? null : engine.register(bmp);
                    JSONObject o = new JSONObject();
                    o.put("ok", r != null && r.ok);
                    o.put("message", r == null ? "フレームを取得できませんでした" : r.message);
                    if (r != null) {
                        o.put("mask", r.mask);
                    }
                    return o.toString();
                }
                case "match": {
                    Bitmap bmp = cameraHost.grabUpright(CAPTURE_WIDTH);
                    SafrEngine.Result r = (bmp == null) ? null : engine.match(bmp);
                    JSONObject o = new JSONObject();
                    o.put("ok", r != null && r.ok);
                    o.put("message", r == null ? "フレームを取得できませんでした" : r.message);
                    if (r != null) {
                        o.put("confidence", r.confidence);
                        o.put("mask", r.mask);
                    }
                    return o.toString();
                }
                case "clearStore": {
                    engine.clearStore();
                    JSONObject o = new JSONObject();
                    o.put("ready", engine.isInitialized());
                    o.put("registered", engine.hasPerson());
                    return o.toString();
                }
                default:
                    return "{}";
            }
        }
    }
}
