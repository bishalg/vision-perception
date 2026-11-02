package org.curv.asd.spike.backend

import android.app.Activity
import android.opengl.GLES11Ext
import android.opengl.GLES20
import android.opengl.GLSurfaceView
import android.view.Surface
import android.widget.FrameLayout
import com.google.ar.core.ArCoreApk
import com.google.ar.core.AugmentedFace
import com.google.ar.core.CameraConfig
import com.google.ar.core.CameraConfigFilter
import com.google.ar.core.Config
import com.google.ar.core.Session
import com.google.ar.core.TrackingState
import com.google.ar.core.exceptions.UnsupportedConfigurationException
import org.json.JSONArray
import org.json.JSONObject
import java.util.EnumSet
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10

/**
 * Back-end 2: ARCore Augmented Faces on the front camera (468-vertex face mesh in metres,
 * centre pose and three region poses in camera space). ARCore runs its own camera session,
 * so this back-end does not use CameraX. Augmented Faces has **no eye, iris or blink
 * output**: whatever this back-end records is head pose and face shape only.
 *
 * Needs a device on the ARCore supported-devices list with front-camera Augmented Faces, and
 * Google Play Services for AR installed (the app offers the install).
 */
class ArCoreBackend : CaptureBackend, GLSurfaceView.Renderer {
    override val id = "arcore"

    private var activity: Activity? = null
    private var session: Session? = null
    private var glView: GLSurfaceView? = null
    private var onError: ((String) -> Unit)? = null
    private var installRequested = false
    private var resumed = false
    private var cameraConfig: CameraConfig? = null
    @Volatile private var sink: FrameSink? = null
    @Volatile private var lastNs = Long.MIN_VALUE
    @Volatile private var intrinsics: JSONObject? = null
    private var textureId = -1
    private var viewportChanged = false
    private var viewW = 0
    private var viewH = 0

    override fun start(activity: Activity, container: FrameLayout, sink: FrameSink, onError: (String) -> Unit) {
        this.activity = activity
        this.sink = sink
        this.onError = onError
        // ARCore renders into a GL surface; nothing of the camera image is drawn (black).
        val gl = GLSurfaceView(activity).apply {
            preserveEGLContextOnPause = true
            setEGLContextClientVersion(2)
            setEGLConfigChooser(8, 8, 8, 8, 16, 0)
            setRenderer(this@ArCoreBackend)
            renderMode = GLSurfaceView.RENDERMODE_CONTINUOUSLY
        }
        container.addView(gl, 0, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        glView = gl
        onResume()
    }

    override fun onResume() {
        val act = activity ?: return
        if (session == null) {
            try {
                val availability = ArCoreApk.getInstance().checkAvailability(act)
                if (availability == ArCoreApk.Availability.UNSUPPORTED_DEVICE_NOT_CAPABLE) {
                    onError?.invoke("This phone is not on the ARCore supported-devices list (record n/a in the sheet).")
                    return
                }
                when (ArCoreApk.getInstance().requestInstall(act, !installRequested)) {
                    ArCoreApk.InstallStatus.INSTALL_REQUESTED -> {
                        if (installRequested) {
                            onError?.invoke("Google Play Services for AR is not installed (install it from the Play Store, or record n/a).")
                        }
                        installRequested = true
                        return
                    }
                    ArCoreApk.InstallStatus.INSTALLED -> Unit
                }
                val s = Session(act, EnumSet.of(Session.Feature.FRONT_CAMERA))
                val filter = CameraConfigFilter(s)
                    .setFacingDirection(CameraConfig.FacingDirection.FRONT)
                    .setTargetFps(EnumSet.of(CameraConfig.TargetFps.TARGET_FPS_30))
                s.getSupportedCameraConfigs(filter).firstOrNull()?.let {
                    s.cameraConfig = it
                    cameraConfig = it
                }
                val config = Config(s).apply {
                    augmentedFaceMode = Config.AugmentedFaceMode.MESH3D
                    updateMode = Config.UpdateMode.BLOCKING
                }
                try {
                    s.configure(config)
                } catch (e: UnsupportedConfigurationException) {
                    s.close()
                    onError?.invoke("ARCore Augmented Faces is not supported on this device (record n/a in the sheet).")
                    return
                }
                session = s
            } catch (e: Exception) {
                onError?.invoke("ARCore unavailable: ${e.javaClass.simpleName}: ${e.message} (record n/a in the sheet).")
                return
            }
        }
        if (resumed) return
        try {
            session?.resume() ?: return
            resumed = true
        } catch (e: Exception) {
            onError?.invoke("ARCore camera could not start: ${e.message}")
            session = null
            return
        }
        glView?.onResume()
    }

    override fun onPause() {
        glView?.onPause()
        session?.pause()
        resumed = false
    }

    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        GLES20.glClearColor(0f, 0f, 0f, 1f)
        val tex = IntArray(1)
        GLES20.glGenTextures(1, tex, 0)
        textureId = tex[0]
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, textureId)
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
    }

    override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
        GLES20.glViewport(0, 0, width, height)
        viewW = width
        viewH = height
        viewportChanged = true
    }

    override fun onDrawFrame(gl: GL10?) {
        GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT or GLES20.GL_DEPTH_BUFFER_BIT)
        val s = session ?: return
        try {
            s.setCameraTextureName(textureId)
            if (viewportChanged) {
                s.setDisplayGeometry(Surface.ROTATION_0, viewW, viewH)
                viewportChanged = false
            }
            val frame = s.update()
            val ns = frame.timestamp
            if (ns == 0L || ns <= lastNs) return
            lastNs = ns
            if (intrinsics == null) {
                val ci = frame.camera.imageIntrinsics
                intrinsics = JSONObject()
                    .put("focalLengthPx", JSONArray(ci.focalLength.map { it.toDouble() }))
                    .put("principalPointPx", JSONArray(ci.principalPoint.map { it.toDouble() }))
                    .put("imageDimensions", JSONArray(ci.imageDimensions.toList()))
            }
            val face = s.getAllTrackables(AugmentedFace::class.java)
                .firstOrNull { it.trackingState == TrackingState.TRACKING }
            sink?.onFrame(Observation(ns, face != null, face != null, null, null, sidecarLine(ns, face)))
        } catch (e: Exception) {
            activity?.runOnUiThread { onError?.invoke("ARCore frame error: ${e.message}") }
        }
    }

    private fun sidecarLine(ns: Long, face: AugmentedFace?): String {
        val sb = StringBuilder(2048)
        sb.append("{\"t_sec\":").append(ns / 1e9).append(",\"frame_ns\":").append(ns)
        sb.append(",\"face\":").append(face != null)
        if (face != null) {
            val m = FloatArray(16)
            face.centerPose.toMatrix(m, 0)
            sb.append(",\"center_pose\":").floats(m)
            for ((key, region) in listOf(
                "nose_tip" to AugmentedFace.RegionType.NOSE_TIP,
                "forehead_left" to AugmentedFace.RegionType.FOREHEAD_LEFT,
                "forehead_right" to AugmentedFace.RegionType.FOREHEAD_RIGHT,
            )) {
                face.getRegionPose(region).toMatrix(m, 0)
                sb.append(",\"").append(key).append("\":").floats(m)
            }
            val v = face.meshVertices
            sb.append(",\"mesh_vertices\":[")
            EyeLandmarks.MESH_468.forEachIndexed { i, k ->
                if (i > 0) sb.append(',')
                sb.append('[').num(v.get(3 * k)).append(',').num(v.get(3 * k + 1)).append(',').num(v.get(3 * k + 2)).append(']')
            }
            sb.append(']')
        }
        return sb.append('}').toString()
    }

    override fun stop() {
        sink = null
        glView?.onPause()
        session?.pause()
        session?.close()
        session = null
        resumed = false
    }

    override fun describe(): JSONObject {
        val o = JSONObject()
            .put("backend", id)
            .put("library", "com.google.ar:core:1.57.0")
            .put("cameraApi", "ARCore Session FRONT_CAMERA, AugmentedFaceMode.MESH3D")
            .put("meshVertexIndices", JSONArray(EyeLandmarks.MESH_468.toList()))
            .put("poseLayout", "column-major 4x4 from Pose.toMatrix, metres, ARCore camera/world space, uninterpreted")
            .put("meshSpace", "AugmentedFace.getMeshVertices, metres, relative to centre pose")
            .put("eyeData", "none (Augmented Faces has no eye, iris or blink output)")
        cameraConfig?.let {
            o.put("cameraConfig", JSONObject()
                .put("imageSize", JSONArray(listOf(it.imageSize.width, it.imageSize.height)))
                .put("fpsRange", JSONArray(listOf(it.fpsRange.lower, it.fpsRange.upper)))
                .put("cameraId", it.cameraId))
        }
        intrinsics?.let { o.put("imageIntrinsics", it) }
        return o
    }
}
