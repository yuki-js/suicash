package jp.serkenn.hicara.suicashui;

import android.util.Log;

import com.viaembedded.smartetk.GPIO;

/**
 * Hi-CARA 上部 RGB LED(SmartETK GPIO)の制御。
 * ピン 1=緑 / 2=赤 / 3=青(frcardreader の LedClass と同じ割当)。
 * setEnable(pin,true) → setValue(pin, 1/0) で個別オン/オフ。混色可。
 * 点滅はアプリ側で 500ms トグル。
 *
 * ローカルデーモン 127.0.0.1:49582 へ TCP する GPIO サービスを使う。
 * サービスが無い/権限が無い環境でも落ちないよう、失敗は握りつぶす。
 */
public final class LedController {

    private static final String TAG = "SuiCashUI";
    private static final int PIN_GREEN = 1;
    private static final int PIN_RED = 2;
    private static final int PIN_BLUE = 3;
    private static final int[] PINS = { PIN_GREEN, PIN_RED, PIN_BLUE };

    public enum Mode {
        OFF,          // 消灯
        BLUE_BLINK,   // 認証中/顔認証(青点滅)
        GREEN,        // 成功(緑点灯)
        RED,          // 失敗/未登録(赤点灯)
        BLUE,         // 待機の淡い在席表示に使う場合(青点灯)
    }

    private GPIO gpio;
    private boolean ready = false;

    private Thread blinkThread;
    private volatile boolean blinking = false;

    public synchronized void init() {
        try {
            gpio = new GPIO();
            for (int pin : PINS) {
                gpio.setEnable(pin, true);
                gpio.setValue(pin, 0);
            }
            ready = true;
            Log.i(TAG, "LED init OK");
        } catch (Throwable t) {
            ready = false;
            Log.w(TAG, "LED init failed (LED無効で続行): " + t);
        }
    }

    /** r/g/b を 0/1 で即時設定(点滅は止める) */
    private synchronized void solid(int r, int g, int b) {
        stopBlink();
        write(r, g, b);
    }

    private void write(int r, int g, int b) {
        if (!ready) return;
        try {
            gpio.setValue(PIN_RED, r);
            gpio.setValue(PIN_GREEN, g);
            gpio.setValue(PIN_BLUE, b);
        } catch (Throwable t) {
            Log.w(TAG, "LED write failed: " + t);
        }
    }

    private synchronized void stopBlink() {
        blinking = false;
        if (blinkThread != null) {
            blinkThread.interrupt();
            blinkThread = null;
        }
    }

    /** 指定色を 500ms 周期で点滅 */
    private synchronized void blink(final int r, final int g, final int b) {
        stopBlink();
        if (!ready) return;
        blinking = true;
        blinkThread = new Thread(() -> {
            boolean on = true;
            while (blinking && !Thread.currentThread().isInterrupted()) {
                write(on ? r : 0, on ? g : 0, on ? b : 0);
                on = !on;
                try {
                    Thread.sleep(500);
                } catch (InterruptedException e) {
                    break;
                }
            }
        });
        blinkThread.start();
    }

    /** UI 状態に対応する LED を設定 */
    public synchronized void set(Mode mode) {
        switch (mode) {
            case OFF:
                solid(0, 0, 0);
                break;
            case GREEN:
                solid(0, 1, 0);
                break;
            case RED:
                solid(1, 0, 0);
                break;
            case BLUE:
                solid(0, 0, 1);
                break;
            case BLUE_BLINK:
                blink(0, 0, 1);
                break;
        }
    }

    /** 文字列(JS ブリッジから)→ Mode */
    public void setByName(String name) {
        Mode m;
        switch (name == null ? "" : name) {
            case "blue_blink": m = Mode.BLUE_BLINK; break;
            case "green":      m = Mode.GREEN; break;
            case "red":        m = Mode.RED; break;
            case "blue":       m = Mode.BLUE; break;
            default:           m = Mode.OFF; break;
        }
        set(m);
    }

    public synchronized void release() {
        stopBlink();
        write(0, 0, 0);
        if (gpio != null) {
            try {
                gpio.closeService();
            } catch (Throwable ignored) {
            }
        }
    }
}
