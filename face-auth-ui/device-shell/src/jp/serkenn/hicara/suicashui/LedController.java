package jp.serkenn.hicara.suicashui;

import android.util.Log;

import com.viaembedded.smartetk.GPIO;

/**
 * Controls the Hi-CARA top RGB LED (SmartETK GPIO).
 * Pins 1=green / 2=red / 3=blue (same mapping as frcardreader's LedClass).
 * setEnable(pin,true) → setValue(pin, 1/0) turns each on/off. Colors can be mixed.
 * Blinking is a 500ms toggle done in the app.
 *
 * Uses the GPIO service that talks TCP to the local daemon at 127.0.0.1:49582.
 * Failures are swallowed so it won't crash when the service or permission is missing.
 */
public final class LedController {

    private static final String TAG = "SuiCashUI";
    private static final int PIN_GREEN = 1;
    private static final int PIN_RED = 2;
    private static final int PIN_BLUE = 3;
    private static final int[] PINS = { PIN_GREEN, PIN_RED, PIN_BLUE };

    public enum Mode {
        OFF,          // off
        BLUE_BLINK,   // authenticating / face auth (blue blink)
        GREEN,        // success (solid green)
        RED,          // failure / not registered (solid red)
        BLUE,         // for a subtle idle presence indicator (solid blue)
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
            Log.w(TAG, "LED init failed (continuing with LED disabled): " + t);
        }
    }

    /** Set r/g/b to 0/1 immediately (stops blinking) */
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

    /** Blink the given color with a 500ms period */
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

    /** Set the LED for the given UI state */
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

    /** String (from the JS bridge) → Mode */
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
