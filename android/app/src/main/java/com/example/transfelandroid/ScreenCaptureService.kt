package com.example.transfelandroid

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.IBinder
import android.view.Surface
import java.io.BufferedOutputStream
import java.net.InetSocketAddress
import java.net.Socket

class ScreenCaptureService : Service() {

    private var mediaProjection: MediaProjection? = null
    private var virtualDisplay: VirtualDisplay? = null

    private var encoder: MediaCodec? = null
    private var inputSurface: Surface? = null

    private var socket: Socket? = null
    private var outputStream: BufferedOutputStream? = null

    private var running = false

    private val mediaProjectionCallback =
        object : MediaProjection.Callback() {

            override fun onStop() {

                enviarEstado(
                    "MEDIAPROJECTION: detenida"
                )

                running = false

                cerrarConexion()

                virtualDisplay?.release()
                virtualDisplay = null
            }
        }

    override fun onCreate() {

        super.onCreate()

        createNotificationChannel()
    }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int
    ): Int {

        enviarEstado(
            "SERVICE: iniciado"
        )

        val notification =
            createNotification()

        try {

            if (android.os.Build.VERSION.SDK_INT >= 29) {

                startForeground(
                    1,
                    notification,
                    android.content.pm.ServiceInfo
                        .FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
                )

            } else {

                @Suppress("DEPRECATION")
                startForeground(
                    1,
                    notification
                )
            }

            enviarEstado(
                "SERVICE: foreground OK"
            )

        } catch (e: Exception) {

            enviarEstado(
                "ERROR FOREGROUND: ${e.message}"
            )

            return START_NOT_STICKY
        }

        try {

            val resultCode =
                intent?.getIntExtra(
                    "resultCode",
                    -1
                ) ?: -1

            val data =
                if (android.os.Build.VERSION.SDK_INT >= 33) {

                    intent?.getParcelableExtra(
                        "data",
                        Intent::class.java
                    )

                } else {

                    @Suppress("DEPRECATION")
                    intent?.getParcelableExtra<Intent>(
                        "data"
                    )
                }

            if (data == null) {

                enviarEstado(
                    "ERROR: DATA NULL"
                )

                stopSelf()

                return START_NOT_STICKY
            }

            enviarEstado(
                "DATA: recibida"
            )

            val projectionManager =
                getSystemService(
                    MEDIA_PROJECTION_SERVICE
                ) as MediaProjectionManager

            mediaProjection =
                projectionManager.getMediaProjection(
                    resultCode,
                    data
                )

            if (mediaProjection == null) {

                enviarEstado(
                    "ERROR: MediaProjection NULL"
                )

                stopSelf()

                return START_NOT_STICKY
            }

            enviarEstado(
                "MEDIAPROJECTION: FUNCIONA"
            )

            mediaProjection?.registerCallback(
                mediaProjectionCallback,
                null
            )

            iniciarEncoder()

        } catch (e: Exception) {

            enviarEstado(
                "ERROR CAPTURA: ${e.message}"
            )

            stopSelf()
        }

        return START_NOT_STICKY
    }

    private fun iniciarEncoder() {

        try {

            val metrics =
                resources.displayMetrics

            val width =
                metrics.widthPixels

            val height =
                metrics.heightPixels

            val fps = 30

            val bitrate = 4_000_000

            enviarEstado(
                "Resolucion: ${width}x${height}"
            )

            enviarEstado(
                "Creando encoder H264..."
            )

            val format =
                MediaFormat.createVideoFormat(
                    MediaFormat.MIMETYPE_VIDEO_AVC,
                    width,
                    height
                )

            format.setInteger(
                MediaFormat.KEY_COLOR_FORMAT,
                MediaCodecInfo.CodecCapabilities
                    .COLOR_FormatSurface
            )

            format.setInteger(
                MediaFormat.KEY_BIT_RATE,
                bitrate
            )

            format.setInteger(
                MediaFormat.KEY_FRAME_RATE,
                fps
            )

            format.setInteger(
                MediaFormat.KEY_I_FRAME_INTERVAL,
                1
            )

            encoder =
                MediaCodec.createEncoderByType(
                    MediaFormat.MIMETYPE_VIDEO_AVC
                )

            encoder?.configure(
                format,
                null,
                null,
                MediaCodec.CONFIGURE_FLAG_ENCODE
            )

            inputSurface =
                encoder?.createInputSurface()

            encoder?.start()

            enviarEstado(
                "H264: encoder iniciado"
            )

            conectarRust()

            iniciarVirtualDisplay(
                width,
                height
            )

            iniciarCodificacion()

        } catch (e: Exception) {

            enviarEstado(
                "ERROR H264: ${e.message}"
            )

            stopSelf()
        }
    }

    private fun conectarRust() {

        Thread {

            try {

                enviarEstado(
                    "TCP: conectando a Rust..."
                )

                val nuevoSocket =
                    Socket()

                nuevoSocket.connect(
                    InetSocketAddress(
                        "127.0.0.1",
                        5000
                    ),
                    5000
                )

                socket = nuevoSocket

                outputStream =
                    BufferedOutputStream(
                        nuevoSocket.getOutputStream()
                    )

                enviarEstado(
                    "TCP: CONECTADO A RUST"
                )

            } catch (e: Exception) {

                enviarEstado(
                    "ERROR TCP: ${e.message}"
                )
            }

        }.start()
    }

    private fun iniciarVirtualDisplay(
        width: Int,
        height: Int
    ) {

        try {

            enviarEstado(
                "Creando VirtualDisplay..."
            )

            virtualDisplay =
                mediaProjection?.createVirtualDisplay(
                    "TransFEL",
                    width,
                    height,
                    resources.displayMetrics.densityDpi,
                    DisplayManager
                        .VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                    inputSurface,
                    null,
                    null
                )

            if (virtualDisplay != null) {

                enviarEstado(
                    "VIRTUALDISPLAY: FUNCIONA"
                )

            } else {

                enviarEstado(
                    "ERROR: VirtualDisplay NULL"
                )
            }

        } catch (e: Exception) {

            enviarEstado(
                "ERROR VIRTUALDISPLAY: ${e.message}"
            )
        }
    }

    private fun iniciarCodificacion() {

        Thread {

            try {

                running = true

                val bufferInfo =
                    MediaCodec.BufferInfo()

                var frames = 0

                enviarEstado(
                    "H264: esperando frames..."
                )

                while (running) {

                    val index =
                        encoder?.dequeueOutputBuffer(
                            bufferInfo,
                            10000
                        ) ?: -1

                    if (index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {

                        val nuevoFormato =
                            encoder?.outputFormat

                        enviarEstado(
                            "H264: formato cambiado"
                        )

                        enviarEstado(
                            "H264: ${nuevoFormato}"
                        )

                    } else if (index >= 0) {

                        val buffer =
                            encoder?.getOutputBuffer(
                                index
                            )

                        if (
                            buffer != null &&
                            bufferInfo.size > 0
                        ) {

                            val datos =
                                ByteArray(
                                    bufferInfo.size
                                )

                            buffer.position(
                                bufferInfo.offset
                            )

                            buffer.limit(
                                bufferInfo.offset +
                                        bufferInfo.size
                            )

                            buffer.get(datos)

                            enviarFrameTCP(
                                datos
                            )

                            frames++

                            if (frames == 1) {

                                enviarEstado(
                                    "H264: PRIMER FRAME ENVIADO"
                                )
                            }

                            if (frames % 30 == 0) {

                                enviarEstado(
                                    "H264: $frames frames enviados"
                                )
                            }
                        }

                        encoder?.releaseOutputBuffer(
                            index,
                            false
                        )
                    }
                }

            } catch (e: Exception) {

                enviarEstado(
                    "ERROR CODIFICADOR: ${e.message}"
                )
            }

        }.start()
    }

    private fun enviarFrameTCP(
        datos: ByteArray
    ) {

        try {

            val salida =
                outputStream

            if (salida == null) {
                return
            }

            val tamaño =
                datos.size

            val tamañoBytes =
                byteArrayOf(
                    ((tamaño shr 24) and 0xFF).toByte(),
                    ((tamaño shr 16) and 0xFF).toByte(),
                    ((tamaño shr 8) and 0xFF).toByte(),
                    (tamaño and 0xFF).toByte()
                )

            synchronized(this) {

                salida.write(
                    tamañoBytes
                )

                salida.write(
                    datos
                )

                salida.flush()
            }

        } catch (e: Exception) {

            enviarEstado(
                "ERROR ENVIANDO FRAME: ${e.message}"
            )

            running = false
        }
    }

    private fun cerrarConexion() {

        try {

            outputStream?.close()

        } catch (_: Exception) {
        }

        try {

            socket?.close()

        } catch (_: Exception) {
        }

        outputStream = null
        socket = null
    }

    private fun enviarEstado(
        mensaje: String
    ) {

        val intent =
            Intent(
                "com.example.transfelandroid.ESTADO"
            )

        intent.setPackage(
            packageName
        )

        intent.putExtra(
            "estado",
            mensaje
        )

        sendBroadcast(intent)
    }

    private fun createNotificationChannel() {

        val channel =
            NotificationChannel(
                "transfel_capture",
                "TransFEL",
                NotificationManager.IMPORTANCE_LOW
            )

        val manager =
            getSystemService(
                NotificationManager::class.java
            )

        manager.createNotificationChannel(
            channel
        )
    }

    private fun createNotification(): Notification {

        return Notification.Builder(
            this,
            "transfel_capture"
        )
            .setContentTitle(
                "TransFEL"
            )
            .setContentText(
                "Capturando pantalla"
            )
            .setSmallIcon(
                android.R.drawable.ic_menu_view
            )
            .build()
    }

    override fun onDestroy() {

        running = false

        cerrarConexion()

        virtualDisplay?.release()

        virtualDisplay = null

        try {
            encoder?.stop()
        } catch (_: Exception) {
        }

        try {
            encoder?.release()
        } catch (_: Exception) {
        }

        encoder = null

        inputSurface?.release()

        inputSurface = null

        mediaProjection?.unregisterCallback(
            mediaProjectionCallback
        )

        mediaProjection?.stop()

        mediaProjection = null

        super.onDestroy()
    }

    override fun onBind(
        intent: Intent?
    ): IBinder? {

        return null
    }
}