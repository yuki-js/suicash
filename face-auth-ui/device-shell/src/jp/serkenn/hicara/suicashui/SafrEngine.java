package jp.serkenn.hicara.suicashui;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Matrix;
import android.util.Log;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

import jp.co.nextware.frcardreader.safr.ConfigOptions;
import jp.co.nextware.frcardreader.safr.EARDetectedFace;
import jp.co.nextware.frcardreader.safr.ESDKWrapper;
import jp.co.nextware.frcardreader.safr.FaceInfo;

/**
 * 端末組み込みの商用顔認証エンジン(SAFR eSDK)を呼び出すヘルパー。
 * エンジンのバイナリ・モデル・ライセンスは再配布不可のため同梱しない
 * (.gitignore 対象。ビルドする本人が自機からローカルに置く)。
 * JNI ラッパー(jp.co.nextware.frcardreader.safr)のクラス名・フィールド名は
 * エンジンのネイティブ側が RegisterNatives で解決する名前なので変更できない。
 *
 *   初期化 : initESDK(LICENSE, モデル展開先, ストア用パス, ConfigOptions)
 *   検出   : detectFaces(bmp, 5)                     … probe(プレビュー用、0°のみ)
 *   登録   : detectFaces(bmp, 5) -> learnPerson      … 多方向リトライあり
 *   照合   : detectFaces(bmp, 5) -> recognizePerson  … 多方向リトライあり
 */
public class SafrEngine {

    private static final String TAG = "SuiCashUI";

    private static final String LICENSE = SafrLicense.LICENSE;
    private static final String MODELS_ASSET = "ESDKModels.zip";
    private static final String MODELS_DIR = "ESDKModels";

    private final ESDKWrapper esdk = new ESDKWrapper();
    private boolean initialized = false;
    private boolean registered = false;

    /** probe の結果(検出のみ。confidence は照合系でのみ入る) */
    public static class Probe {
        public final boolean found;
        public final EARDetectedFace face; // found のときのみ
        public final int rotation;         // 検出に使った追加回転(probe では常に 0)

        Probe(boolean found, EARDetectedFace face, int rotation) {
            this.found = found;
            this.face = face;
            this.rotation = rotation;
        }
    }

    /** register/match の結果 */
    public static class Result {
        public final boolean ok;
        public final double confidence;
        public final float mask;
        public final String message;

        Result(boolean ok, double confidence, float mask, String message) {
            this.ok = ok;
            this.confidence = confidence;
            this.mask = mask;
            this.message = message;
        }
    }

    public synchronized boolean isInitialized() {
        return initialized;
    }

    public synchronized boolean hasPerson() {
        return registered;
    }

    /** モデル展開 + native 初期化。数十秒かかるのでバックグラウンドで呼ぶこと */
    public synchronized boolean init(Context ctx) {
        if (initialized) {
            return true;
        }
        try {
            String modelsPath = prepareModels(ctx);
            String storePath = new File(ctx.getCacheDir(), "store").getPath();
            new File(storePath).mkdirs();

            ConfigOptions cfg = ConfigOptions.Default();
            cfg.faceRecognitionModelType = ConfigOptions.kEARFaceRecognitionModelType_Regular;
            cfg.recognitionModelPerformance = ConfigOptions.kEARFaceRecognitionModelPerformance_AccuracyOptimized;

            long rc = esdk.initESDK(LICENSE, modelsPath, storePath, cfg);
            Log.i(TAG, "initESDK rc=" + rc + " models=" + modelsPath);
            initialized = (rc == 0);
            return initialized;
        } catch (Throwable t) {
            Log.e(TAG, "init failed", t);
            return false;
        }
    }

    /**
     * プレビュー用: 検出だけ行い、最大の顔の品質値を返す。
     * 低頻度で連続して呼ぶ前提のため、多方向リトライはしない(0° のみ)。
     */
    public synchronized Probe probe(Bitmap bmp) {
        if (!initialized) {
            return new Probe(false, null, 0);
        }
        try {
            ArrayList<FaceInfo> detected = esdk.detectFaces(bmp, 5);
            if (detected.isEmpty()) {
                return new Probe(false, null, 0);
            }
            FaceInfo best = detected.get(0);
            double bestArea = area(best.detectedFace);
            for (FaceInfo fi : detected) {
                double a = area(fi.detectedFace);
                if (a > bestArea) {
                    best = fi;
                    bestArea = a;
                }
            }
            return new Probe(true, best.detectedFace, 0);
        } catch (Throwable t) {
            Log.e(TAG, "probe failed", t);
            return new Probe(false, null, 0);
        }
    }

    private static double area(EARDetectedFace f) {
        return f.boundsWidth * f.boundsHeight;
    }

    private static final int[] RETRY_ROTATIONS = {0, 90, 270, 180};

    /** 顔が検出できる向きの Bitmap を返す(見つからなければ null)。検出キャッシュはその向きになる */
    private Bitmap orientForFace(Bitmap bmp) {
        for (int deg : RETRY_ROTATIONS) {
            Bitmap cand = (deg == 0) ? bmp : rotate(bmp, deg);
            ArrayList<FaceInfo> detected = esdk.detectFaces(cand, 5);
            if (!detected.isEmpty()) {
                if (deg != 0) {
                    Log.i(TAG, "顔を追加回転 " + deg + "° で検出");
                }
                return cand;
            }
        }
        return null;
    }

    private static Bitmap rotate(Bitmap src, int deg) {
        Matrix m = new Matrix();
        m.postRotate(deg);
        return Bitmap.createBitmap(src, 0, 0, src.getWidth(), src.getHeight(), m, true);
    }

    /** 登録。ストアをクリアしてから 1 件登録する(常に最後の 1 人だけ) */
    public synchronized Result register(Bitmap bmp) {
        if (!initialized) {
            return new Result(false, 0, 0, "未初期化");
        }
        try {
            Bitmap oriented = orientForFace(bmp);
            if (oriented == null) {
                return new Result(false, 0, 0, "顔を検出できませんでした");
            }
            esdk.clearPersonStore();
            registered = false;
            ArrayList<FaceInfo> learned = esdk.learnPerson(oriented, 5);
            if (learned.isEmpty()) {
                return new Result(false, 0, 0, "顔の登録に失敗しました");
            }
            registered = true;
            float mask = learned.get(0).detectedFace.mask;
            return new Result(true, 0, mask, "登録しました");
        } catch (Throwable t) {
            Log.e(TAG, "register failed", t);
            return new Result(false, 0, 0, "登録エラー: " + t.getMessage());
        }
    }

    /** 照合。confidence(類似度。1.0 超あり)を返す */
    public synchronized Result match(Bitmap bmp) {
        if (!initialized) {
            return new Result(false, 0, 0, "未初期化");
        }
        try {
            Bitmap oriented = orientForFace(bmp);
            if (oriented == null) {
                return new Result(false, 0, 0, "顔を検出できませんでした");
            }
            ArrayList<FaceInfo> recognized = esdk.recognizePerson(oriented, 5);
            if (recognized.isEmpty()) {
                return new Result(false, 0, 0, "照合できませんでした(登録済みの顔がない可能性)");
            }
            EARDetectedFace f = recognized.get(0).detectedFace;
            return new Result(true, f.confidence, f.mask, "照合しました");
        } catch (Throwable t) {
            Log.e(TAG, "match failed", t);
            return new Result(false, 0, 0, "照合エラー: " + t.getMessage());
        }
    }

    public synchronized void clearStore() {
        if (initialized) {
            try {
                esdk.clearPersonStore();
            } catch (Throwable ignored) {
            }
            registered = false;
        }
    }

    // ------------------------------------------------------------- モデル展開

    /**
     * assets/ESDKModels.zip を filesDir 直下に展開し、filesDir/ESDKModels を返す。
     * zip は中身が "ESDKModels/xxx" 形式なので展開先は filesDir ルート
     * (ESDKModels/ に展開すると二重になり initESDK が rc=12 で失敗する)。
     */
    private String prepareModels(Context ctx) throws Exception {
        File root = ctx.getFilesDir();
        File modelsDir = new File(root, MODELS_DIR);
        File marker = new File(modelsDir, ".unzipped");
        if (marker.exists()) {
            return modelsDir.getAbsolutePath();
        }
        byte[] buf = new byte[64 * 1024];
        try (InputStream ain = ctx.getAssets().open(MODELS_ASSET);
             ZipInputStream zin = new ZipInputStream(ain)) {
            ZipEntry e;
            while ((e = zin.getNextEntry()) != null) {
                File out = new File(root, e.getName());
                // Zip Slip 対策
                if (!out.getCanonicalPath().startsWith(root.getCanonicalPath() + File.separator)) {
                    throw new SecurityException("不正なzipエントリ: " + e.getName());
                }
                if (e.isDirectory()) {
                    out.mkdirs();
                } else {
                    File parent = out.getParentFile();
                    if (parent != null) {
                        parent.mkdirs();
                    }
                    try (OutputStream os = new FileOutputStream(out)) {
                        int n;
                        while ((n = zin.read(buf)) != -1) {
                            os.write(buf, 0, n);
                        }
                    }
                }
                zin.closeEntry();
            }
        }
        modelsDir.mkdirs();
        new FileOutputStream(marker).close();
        Log.i(TAG, "models extracted to " + modelsDir.getAbsolutePath());
        return modelsDir.getAbsolutePath();
    }
}
