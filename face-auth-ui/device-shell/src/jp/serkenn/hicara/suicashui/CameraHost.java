package jp.serkenn.hicara.suicashui;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Matrix;
import android.graphics.SurfaceTexture;
import android.hardware.camera2.CameraAccessException;
import android.hardware.camera2.CameraCaptureSession;
import android.hardware.camera2.CameraCharacteristics;
import android.hardware.camera2.CameraDevice;
import android.hardware.camera2.CameraManager;
import android.hardware.camera2.CaptureRequest;
import android.hardware.camera2.params.StreamConfigurationMap;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.util.Size;
import android.view.Surface;
import android.view.TextureView;

import java.util.Arrays;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Front camera (Camera2) host. Streams the preview to a TextureView placed under
 * the WebView, and grabUpright() returns the current frame as an upright Bitmap.
 *
 * WebView (Chromium 74) getUserMedia doesn't work with this device's camera HAL
 * and delivers no frames, so the camera is owned natively.
 *
 * This device reports SENSOR_ORIENTATION as 0 but the sensor is actually mounted sideways,
 * so BASE_ROTATION=90 makes it upright (fixed portrait use).
 */
public class CameraHost {

    private static final String TAG = "SuiCashUI";
    private static final int BASE_ROTATION = 90;

    private final Context context;
    private final TextureView view;
    private final Handler main = new Handler(Looper.getMainLooper());

    private CameraDevice camera;
    private CameraCaptureSession session;
    private Size previewSize;
    private volatile boolean wanted = false;

    public CameraHost(Context context, TextureView view) {
        this.context = context;
        this.view = view;
        view.setSurfaceTextureListener(new TextureView.SurfaceTextureListener() {
            @Override
            public void onSurfaceTextureAvailable(SurfaceTexture st, int w, int h) {
                if (wanted) {
                    openCamera();
                }
            }

            @Override
            public void onSurfaceTextureSizeChanged(SurfaceTexture st, int w, int h) {
            }

            @Override
            public boolean onSurfaceTextureDestroyed(SurfaceTexture st) {
                return true;
            }

            @Override
            public void onSurfaceTextureUpdated(SurfaceTexture st) {
            }
        });
    }

    /** Start the camera (delegated to the UI thread). If the TextureView isn't ready, open once it is */
    public void start() {
        wanted = true;
        main.post(this::openCamera);
    }

    /** Stop the camera */
    public void stop() {
        wanted = false;
        main.post(this::closeCamera);
    }

    public boolean isRunning() {
        return session != null;
    }

    /**
     * Return the current frame as an upright Bitmap (call from a worker thread).
     * getBitmap is UI-thread only, so hand it over via a latch.
     * Grab at full camera resolution, rotate, then downscale if needed (face detection is sensitive to orientation and aspect).
     */
    public Bitmap grabUpright(int maxW) {
        final AtomicReference<Bitmap> ref = new AtomicReference<>();
        final CountDownLatch latch = new CountDownLatch(1);
        main.post(() -> {
            try {
                if (view.isAvailable() && previewSize != null) {
                    ref.set(view.getBitmap(previewSize.getWidth(), previewSize.getHeight()));
                }
            } catch (Throwable t) {
                Log.w(TAG, "getBitmap failed", t);
            } finally {
                latch.countDown();
            }
        });
        try {
            if (!latch.await(2, TimeUnit.SECONDS)) {
                return null;
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return null;
        }
        Bitmap raw = ref.get();
        if (raw == null) {
            return null;
        }
        Matrix m = new Matrix();
        m.postRotate(BASE_ROTATION);
        Bitmap up = Bitmap.createBitmap(raw, 0, 0, raw.getWidth(), raw.getHeight(), m, true);
        if (maxW > 0 && up.getWidth() > maxW) {
            int h = Math.round((float) up.getHeight() * maxW / up.getWidth());
            up = Bitmap.createScaledBitmap(up, maxW, h, true);
        }
        return up;
    }

    // ------------------------------------------------------------ Camera2

    @SuppressLint("MissingPermission") // CAMERA already checked in MainActivity
    private void openCamera() {
        if (!wanted || camera != null || !view.isAvailable()) {
            return;
        }
        try {
            CameraManager cm = (CameraManager) context.getSystemService(Context.CAMERA_SERVICE);
            String frontId = null;
            for (String id : cm.getCameraIdList()) {
                CameraCharacteristics cc = cm.getCameraCharacteristics(id);
                Integer facing = cc.get(CameraCharacteristics.LENS_FACING);
                if (facing != null && facing == CameraCharacteristics.LENS_FACING_FRONT) {
                    frontId = id;
                    previewSize = choosePreviewSize(cc);
                    break;
                }
            }
            if (frontId == null || previewSize == null) {
                Log.e(TAG, "front camera not found");
                return;
            }
            Log.i(TAG, "openCamera id=" + frontId + " preview=" + previewSize);
            cm.openCamera(frontId, new CameraDevice.StateCallback() {
                @Override
                public void onOpened(CameraDevice cd) {
                    camera = cd;
                    if (!wanted) {
                        closeCamera();
                        return;
                    }
                    startSession();
                }

                @Override
                public void onDisconnected(CameraDevice cd) {
                    cd.close();
                    camera = null;
                }

                @Override
                public void onError(CameraDevice cd, int error) {
                    Log.e(TAG, "camera error " + error);
                    cd.close();
                    camera = null;
                }
            }, main);
        } catch (CameraAccessException | SecurityException e) {
            Log.e(TAG, "openCamera failed", e);
        }
    }

    private Size choosePreviewSize(CameraCharacteristics cc) {
        StreamConfigurationMap map = cc.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
        if (map == null) {
            return null;
        }
        Size[] sizes = map.getOutputSizes(SurfaceTexture.class);
        if (sizes == null || sizes.length == 0) {
            return null;
        }
        // pick the largest 4:3 size up to 1280 (else the first)
        Size best = null;
        for (Size s : sizes) {
            if (s.getWidth() > 1280) {
                continue;
            }
            if (s.getWidth() * 3 != s.getHeight() * 4) {
                continue;
            }
            if (best == null || s.getWidth() > best.getWidth()) {
                best = s;
            }
        }
        if (best == null) {
            best = sizes[0];
        }
        return best;
    }

    private void startSession() {
        try {
            SurfaceTexture st = view.getSurfaceTexture();
            st.setDefaultBufferSize(previewSize.getWidth(), previewSize.getHeight());
            Surface surface = new Surface(st);
            final CaptureRequest.Builder req =
                    camera.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW);
            req.addTarget(surface);
            camera.createCaptureSession(Arrays.asList(surface),
                    new CameraCaptureSession.StateCallback() {
                        @Override
                        public void onConfigured(CameraCaptureSession s) {
                            if (camera == null) {
                                return;
                            }
                            session = s;
                            try {
                                s.setRepeatingRequest(req.build(), null, main);
                                Log.i(TAG, "camera preview started");
                            } catch (CameraAccessException e) {
                                Log.e(TAG, "setRepeatingRequest failed", e);
                            }
                        }

                        @Override
                        public void onConfigureFailed(CameraCaptureSession s) {
                            Log.e(TAG, "createCaptureSession failed");
                        }
                    }, main);
        } catch (CameraAccessException e) {
            Log.e(TAG, "startSession failed", e);
        }
    }

    private void closeCamera() {
        if (session != null) {
            try {
                session.close();
            } catch (Throwable ignored) {
            }
            session = null;
        }
        if (camera != null) {
            camera.close();
            camera = null;
        }
    }
}
