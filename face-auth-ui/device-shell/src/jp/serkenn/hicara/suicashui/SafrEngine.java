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
 * Helper that calls the device's built-in commercial face-recognition engine (SAFR eSDK).
 * The engine binaries, models and license are not redistributable and are not bundled
 * (gitignored; whoever builds places them locally from their own device).
 * The JNI wrapper (jp.co.nextware.frcardreader.safr) class and field names
 * are resolved by the engine's native side via RegisterNatives, so they cannot change.
 *
 *   Init     : initESDK(LICENSE, model dir, store path, ConfigOptions)
 *   Detect   : detectFaces(bmp, 5)                     … probe (for preview, 0° only)
 *   Register : detectFaces(bmp, 5) -> learnPerson      … retries multiple orientations
 *   Match    : detectFaces(bmp, 5) -> recognizePerson  … retries multiple orientations
 */
public class SafrEngine {

    private static final String TAG = "SuiCashUI";

    private static final String LICENSE = SafrLicense.LICENSE;
    private static final String MODELS_ASSET = "ESDKModels.zip";
    private static final String MODELS_DIR = "ESDKModels";

    private final ESDKWrapper esdk = new ESDKWrapper();
    private boolean initialized = false;
    private boolean registered = false;

    /** Probe result (detection only; confidence is set only for matching) */
    public static class Probe {
        public final boolean found;
        public final EARDetectedFace face; // only when found
        public final int rotation;         // extra rotation used for detection (always 0 for probe)

        Probe(boolean found, EARDetectedFace face, int rotation) {
            this.found = found;
            this.face = face;
            this.rotation = rotation;
        }
    }

    /** register/match result */
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

    /** Model extraction + native init. Takes tens of seconds, so call in the background */
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
     * For preview: detect only and return the quality of the largest face.
     * Meant to be called repeatedly at low rate, so no multi-orientation retry (0° only).
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

    /** Return the Bitmap in an orientation where a face is detected (null if none). The detection cache ends up in that orientation */
    private Bitmap orientForFace(Bitmap bmp) {
        for (int deg : RETRY_ROTATIONS) {
            Bitmap cand = (deg == 0) ? bmp : rotate(bmp, deg);
            ArrayList<FaceInfo> detected = esdk.detectFaces(cand, 5);
            if (!detected.isEmpty()) {
                if (deg != 0) {
                    Log.i(TAG, "Face detected with extra rotation " + deg + "°");
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

    /** Register. Clears the store and then registers one entry (always only the last person) */
    public synchronized Result register(Bitmap bmp) {
        if (!initialized) {
            return new Result(false, 0, 0, "Not initialized");
        }
        try {
            Bitmap oriented = orientForFace(bmp);
            if (oriented == null) {
                return new Result(false, 0, 0, "No face detected");
            }
            esdk.clearPersonStore();
            registered = false;
            ArrayList<FaceInfo> learned = esdk.learnPerson(oriented, 5);
            if (learned.isEmpty()) {
                return new Result(false, 0, 0, "Failed to register face");
            }
            registered = true;
            float mask = learned.get(0).detectedFace.mask;
            return new Result(true, 0, mask, "Registered");
        } catch (Throwable t) {
            Log.e(TAG, "register failed", t);
            return new Result(false, 0, 0, "Registration error: " + t.getMessage());
        }
    }

    /** Match. Returns confidence (similarity; may exceed 1.0) */
    public synchronized Result match(Bitmap bmp) {
        if (!initialized) {
            return new Result(false, 0, 0, "Not initialized");
        }
        try {
            Bitmap oriented = orientForFace(bmp);
            if (oriented == null) {
                return new Result(false, 0, 0, "No face detected");
            }
            ArrayList<FaceInfo> recognized = esdk.recognizePerson(oriented, 5);
            if (recognized.isEmpty()) {
                return new Result(false, 0, 0, "No match (possibly no registered face)");
            }
            EARDetectedFace f = recognized.get(0).detectedFace;
            return new Result(true, f.confidence, f.mask, "Matched");
        } catch (Throwable t) {
            Log.e(TAG, "match failed", t);
            return new Result(false, 0, 0, "Match error: " + t.getMessage());
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

    // ------------------------------------------------------- model extraction

    /**
     * Extract assets/ESDKModels.zip directly into filesDir and return filesDir/ESDKModels.
     * The zip entries are "ESDKModels/xxx", so extract to the filesDir root
     * (extracting into ESDKModels/ nests it twice and initESDK fails with rc=12).
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
                // Zip Slip protection
                if (!out.getCanonicalPath().startsWith(root.getCanonicalPath() + File.separator)) {
                    throw new SecurityException("Invalid zip entry: " + e.getName());
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
