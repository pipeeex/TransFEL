package com.example.transfelandroid

import android.app.Notification
import android.app.PendingIntent
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.drawable.Icon
import android.hardware.display.VirtualDisplay
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.IBinder
import android.view.Surface
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.DataInputStream
import java.net.InetSocketAddress
import java.net.Socket
import android.hardware.display.DisplayManager
import android.os.Build

class ScreenCaptureService : Service() {
    companion object {
        const val ACTION_ESTADO = "com.example.transfelandroid.ESTADO"
        const val ACTION_PAUSAR = "com.example.transfelandroid.PAUSAR"
        const val ACTION_TERMINAR = "com.example.transfelandroid.TERMINAR"

        const val EXTRA_TIPO = "tipo"
        const val EXTRA_MENSAJE = "estado"

        const val TIPO_LOG = "LOG"
        const val TIPO_CONECTANDO = "CONECTANDO"
        const val TIPO_TRANSMITIENDO = "TRANSMITIENDO"
        const val TIPO_PAUSADA = "PAUSADA"
        const val TIPO_TERMINADA = "TERMINADA"
        const val TIPO_ERROR = "ERROR"
        const val TAG_RESIZE = -65535   // 0xFFFF0001: tamaño del video
        const val TAG_CROP = -65534     // 0xFFFF0002: area util dentro del video

        // Eventos que llegan del PC (control remoto).
        const val CTRL_RUEDA = 0x0105
        const val CTRL_ABAJO = 0x0101
        const val CTRL_MOVER = 0x0102
        const val CTRL_ARRIBA = 0x0103
        const val CTRL_ACCION = 0x0104   // atras / inicio / recientes

        /** La UI lo consulta al volver a primer plano para resincronizarse. */
        @Volatile
        var activo: Boolean = false
            private set

        /** Ultimo estado emitido, para que la UI se reenganche sin esperar eventos. */
        @Volatile
        var ultimoTipo: String = TIPO_TERMINADA
            private set

        @Volatile
        var ultimoMensaje: String = ""
            private set
    }
    private var mediaProjection: MediaProjection? = null
    private var virtualDisplay: VirtualDisplay? = null

    private var encoder: MediaCodec? = null
    private var inputSurface: Surface? = null

    private var socket: Socket? = null
    private var outputStream: BufferedOutputStream? = null

    private var running = false

    private var transmisionPausada = false

    private var anchoActual = 0
    private var altoActual = 0
    private var reiniciando = false
    private var lado = 0   // lado del cuadrado de captura
    private var ultimaRotacion = 0L

    @Volatile
    private var pedirKeyframe = false

    /** Dimensiones actuales, redondeadas a par (H.264 lo exige). */
    /** Tamaño actual de la pantalla, alineado a multiplo de 16 (lo que prefieren los encoders). */
    /**
     * Tamaño real del display. UNA sola fuente: mezclar getRealMetrics con
     * resources.displayMetrics hacia que durante el giro se leyeran
     * orientaciones distintas en lecturas consecutivas, y eso disparaba
     * un bucle infinito de rotaciones.
     */
    private fun tamanoPantalla(): Pair<Int, Int>? {
        return try {
            val dm = android.util.DisplayMetrics()
            val gestor = getSystemService(DisplayManager::class.java)
            val display = gestor?.getDisplay(android.view.Display.DEFAULT_DISPLAY)
                ?: return null

            @Suppress("DEPRECATION")
            display.getRealMetrics(dm)

            if (dm.widthPixels <= 0 || dm.heightPixels <= 0) {
                null
            } else {
                alinear(dm.widthPixels) to alinear(dm.heightPixels)
            }
        } catch (e: Exception) {
            null
        }
    }

    /** Solo para el arranque, donde si o si hace falta un valor. */
    private fun tamanoInicial(): Pair<Int, Int> {
        tamanoPantalla()?.let { return it }
        val m = resources.displayMetrics
        return alinear(m.widthPixels) to alinear(m.heightPixels)
    }

    private fun alinear(valor: Int): Int = (valor / 16) * 16
    private fun enteroABytes(valor: Int): ByteArray = byteArrayOf(
        ((valor shr 24) and 0xFF).toByte(),
        ((valor shr 16) and 0xFF).toByte(),
        ((valor shr 8) and 0xFF).toByte(),
        (valor and 0xFF).toByte(),
    )

    /** Avisa al PC que zona del cuadrado contiene la pantalla real. */
    private fun enviarCrop(width: Int, height: Int) {
        val salida = outputStream ?: return
        try {
            synchronized(this) {
                salida.write(enteroABytes(TAG_CROP))
                salida.write(enteroABytes(width))
                salida.write(enteroABytes(height))
                salida.flush()
            }
        } catch (e: Exception) {
            enviarEstado("No se pudo avisar de la rotacion: ${e.message}")
        }
    }
    private val pauseReceiver = object : BroadcastReceiver() {

            override fun onReceive(
                context: Context?,
                intent: Intent?
            ) {

                if (intent?.action == ACTION_PAUSAR) {
                    if (!running) {
                        enviarEstado("Ignorado: no hay transmision activa")
                        return
                    }

                    transmisionPausada = intent.getBooleanExtra("pausada", false)

                    if (transmisionPausada) {
                        enviarEvento(TIPO_PAUSADA, "Transmision pausada")
                    } else {
                        enviarEvento(TIPO_TRANSMITIENDO, "Transmision reanudada")
                    }

                    actualizarNotificacion()
                }
            }
        }

    private val terminarReceiver = object : BroadcastReceiver() {

            override fun onReceive(
                context: Context?,
                intent: Intent?
            ) {

                if (intent?.action == ACTION_TERMINAR) {
                    if (!activo) {
                        enviarEvento(TIPO_TERMINADA, "No habia nada que detener")
                        return
                    }
                    enviarEstado("Deteniendo...")
                    detenerTransmision()
                }
            }
        }

    private val mediaProjectionCallback = object : MediaProjection.Callback() {

            override fun onStop() {

                activo = false
                enviarEvento(TIPO_TERMINADA, "El sistema detuvo la captura")

                cerrarConexion()

                virtualDisplay?.release()

                virtualDisplay = null
            }
        }

    override fun onCreate() {

        super.onCreate()

        createNotificationChannel()

        val pauseFilter = IntentFilter(
                "com.example.transfelandroid.PAUSAR")

        registerReceiver(
            pauseReceiver,
            pauseFilter,
            Context.RECEIVER_NOT_EXPORTED
        )

        val terminarFilter =
            IntentFilter(
                "com.example.transfelandroid.TERMINAR"
            )

        registerReceiver(
            terminarReceiver,
            terminarFilter,
            Context.RECEIVER_NOT_EXPORTED
        )
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {

        enviarEstado(
            "SERVICE: iniciado"
        )

        val notification =
            createNotification()

        try {

            if (
                android.os.Build.VERSION.SDK_INT >= 29
            ) {

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
            activo = true
            enviarEvento(TIPO_CONECTANDO, "Preparando captura")

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
                if (
                    android.os.Build.VERSION.SDK_INT >= 33
                ) {

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
                enviarEvento(TIPO_ERROR, "No se pudo iniciar la captura")

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

            detenerTransmision()
        }

        return START_NOT_STICKY
    }

    private fun iniciarEncoder() {
        val (w, h) = tamanoInicial()

        // Capturamos en un cuadrado del lado mayor. Asi AUTO_MIRROR encaja
        // la pantalla en cualquier orientacion sin tocar nada: la rotacion
        // deja de requerir reiniciar encoder ni VirtualDisplay.
        lado = alinear(maxOf(w, h))
        anchoActual = w
        altoActual = h

        enviarEstado("Captura: cuadrado ${lado}x${lado}, pantalla ${w}x${h}")

        // El pipeline arranca SOLO cuando el socket esta listo: los primeros
        // bytes del encoder son SPS/PPS y sin ellos FFmpeg no decodifica.
        conectarRust(w, h)
    }
    private fun crearPipeline(width: Int, height: Int) {
        try {
            enviarEstado("Pipeline: ${width}x${height}")

            val format = MediaFormat.createVideoFormat(
                MediaFormat.MIMETYPE_VIDEO_AVC, width, height,
            )
            format.setInteger(
                MediaFormat.KEY_COLOR_FORMAT,
                MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface,
            )
            format.setInteger(MediaFormat.KEY_BIT_RATE, 4_000_000)
            format.setInteger(MediaFormat.KEY_FRAME_RATE, 30)
            format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 1)
            format.setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, 100_000L)

            encoder = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
            enviarEstado("Pipeline: codec creado")

            encoder?.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            enviarEstado("Pipeline: configurado")

            inputSurface = encoder?.createInputSurface()
            encoder?.start()
            enviarEstado("Pipeline: encoder iniciado")

            conectarVirtualDisplay(width, height)
            enviarEstado("Pipeline: display listo")

            iniciarCodificacion()

        } catch (e: Exception) {
            enviarEvento(TIPO_ERROR, "Fallo en pipeline (${width}x${height}): ${e.message}")
            detenerTransmision()
        }
    }

    /**
     * La rotacion ya no reinicia encoder ni VirtualDisplay: la superficie es
     * cuadrada y AUTO_MIRROR reencaja la pantalla solo. Solo hay que decirle
     * al PC que zona del cuadrado mirar.
     */
    @Synchronized
    private fun actualizarOrientacion(width: Int, height: Int) {
        if (width == anchoActual && height == altoActual) return

        enviarEstado("Rotacion: ${anchoActual}x${altoActual} -> ${width}x${height}")

        anchoActual = width
        altoActual = height

        enviarCrop(width, height)

        // El keyframe lo pide el propio hilo de codificacion: MediaCodec no
        // es seguro de tocar desde otro hilo mientras esta en dequeue.
        pedirKeyframe = true
    }

    private fun conectarRust(width: Int, height: Int) {
        Thread {
            try {
                enviarEstado("TCP: conectando al PC...")

                val nuevoSocket = Socket()
                nuevoSocket.connect(InetSocketAddress("127.0.0.1", 5000), 5000)
                nuevoSocket.tcpNoDelay = true
                socket = nuevoSocket

                val salida = BufferedOutputStream(nuevoSocket.getOutputStream())

                // 1) tamaño real del video: el cuadrado.
                salida.write(enteroABytes(TAG_RESIZE))
                salida.write(enteroABytes(lado))
                salida.write(enteroABytes(lado))
                // 2) zona util dentro de ese cuadrado.
                salida.write(enteroABytes(TAG_CROP))
                salida.write(enteroABytes(width))
                salida.write(enteroABytes(height))
                salida.flush()

                // Recien ahora los frames pueden salir.
                outputStream = salida

                // Canal de vuelta: el PC manda los eventos de control por el
                // mismo socket. TCP es full-duplex, no hace falta otro puerto.
                escucharControl(nuevoSocket)

                enviarEvento(TIPO_CONECTANDO, "Conectado al PC, esperando video")

                // Ahora si: encoder + VirtualDisplay, con el socket ya abierto.
                crearPipeline(lado, lado)

            } catch (e: Exception) {
                enviarEvento(TIPO_ERROR, "No se pudo conectar al PC. ¿Esta abierto TransFEL?")
            }
        }.start()
    }

    /** Lee los eventos de control que envia el PC por el mismo socket. */
    private fun escucharControl(socketActivo: java.net.Socket) {
        Thread {
            try {
                val entrada = DataInputStream(
                    BufferedInputStream(socketActivo.getInputStream())
                )

                enviarEstado("Control: escuchando al PC")

                while (!socketActivo.isClosed) {
                    val tag = entrada.readInt()

                    when (tag) {
                        CTRL_RUEDA -> {
                            val x = entrada.readInt().toFloat()
                            val y = entrada.readInt().toFloat()
                            val delta = entrada.readInt().toFloat()
                            ControlService.instancia?.rueda(x, y, delta)
                        }
                        CTRL_ABAJO, CTRL_MOVER, CTRL_ARRIBA -> {
                            val x = entrada.readInt().toFloat()
                            val y = entrada.readInt().toFloat()

                            val control = ControlService.instancia
                            if (control == null) {
                                // Sin servicio de accesibilidad no hay nada que hacer.
                                continue
                            }

                            when (tag) {
                                CTRL_ABAJO -> control.abajo(x, y)
                                CTRL_MOVER -> control.mover(x, y)
                                CTRL_ARRIBA -> control.arriba(x, y)
                            }
                        }

                        CTRL_ACCION -> {
                            val codigo = entrada.readInt()
                            ControlService.instancia?.accionGlobal(codigo)
                        }

                        else -> {
                            enviarEstado("Control: paquete desconocido ($tag)")
                        }
                    }
                }

            } catch (e: Exception) {
                if (running) {
                    enviarEstado("Control: canal cerrado (${e.message})")
                }
            }

            ControlService.instancia?.cancelar()
        }.start()
    }

    private fun conectarVirtualDisplay(width: Int, height: Int) {
        try {
            if (virtualDisplay != null) {
                enviarEstado("VirtualDisplay: ya existe, se reutiliza")
                return
            }

            enviarEstado("Creando VirtualDisplay ${width}x${height}...")

            virtualDisplay = mediaProjection?.createVirtualDisplay(
                "TransFEL",
                width,
                height,
                resources.displayMetrics.densityDpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                inputSurface,
                null,
                null,
            )

            if (virtualDisplay == null) {
                enviarEstado("ERROR: VirtualDisplay NULL")
            } else {
                enviarEstado("VIRTUALDISPLAY: FUNCIONA")
            }

        } catch (e: Exception) {
            enviarEstado("ERROR VIRTUALDISPLAY: ${e.message}")
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

                    val codec = encoder ?: break

                    if (pedirKeyframe) {
                        pedirKeyframe = false
                        runCatching {
                            val params = android.os.Bundle()
                            params.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0)
                            codec.setParameters(params)
                        }.onFailure {
                            enviarEstado("No se pudo pedir keyframe: ${it.message}")
                        }
                    }

                    val index = codec.dequeueOutputBuffer(bufferInfo, 10000)

                    if (index ==MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {

                        val nuevoFormato = codec.outputFormat

                        enviarEstado(
                            "H264: formato cambiado"
                        )

                        enviarEstado(
                            "H264: ${nuevoFormato}"
                        )

                    } else if (index >= 0) {

                        val buffer =
                            codec.getOutputBuffer(
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

                            val esConfig =
                                (bufferInfo.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG) != 0
                            if (esConfig) {
                                enviarEstado("H264: SPS/PPS (${datos.size} bytes)")
                            }

                            if (
                                !transmisionPausada &&
                                running
                            ) {

                                enviarFrameTCP(
                                    datos
                                )

                                frames++

                                if (frames == 1) {

                                    enviarEvento(TIPO_TRANSMITIENDO, "Transmitiendo")

                                }

                                if (
                                    frames % 30 == 0
                                ) {

                                    enviarEstado(
                                        "H264: $frames frames enviados"
                                    )
                                }
                            }
                        }

                        codec.releaseOutputBuffer(
                            index,
                            false
                        )
                    }
                }

            } catch (e: Exception) {

                if (running) {
                    enviarEvento(TIPO_ERROR, "ERROR CODIFICADOR: ${e.message}")
                } else {
                    enviarEstado("Codificador detenido: ${e.message}")
                }
            }

            enviarEstado("Hilo de codificacion terminado (running=$running)")

        }.start()
    }

    private fun enviarFrameTCP(datos: ByteArray) {

        try {

            val salida = outputStream

            if (salida == null) {
                return
            }

            val tamaño = datos.size

            val tamañoBytes = byteArrayOf(
                    ((tamaño shr 24) and 0xFF).toByte(),
                    ((tamaño shr 16) and 0xFF).toByte(),
                    ((tamaño shr 8) and 0xFF).toByte(),
                    (tamaño and 0xFF).toByte()
            )

            synchronized(this) {
                salida.write(tamañoBytes)
                salida.write(datos)
                salida.flush()   // imprescindible en streaming en vivo
            }

        } catch (e: Exception) {
            enviarEvento(TIPO_ERROR, "Se perdio la conexion con el PC")
            running = false
        }
    }
    

    private fun cerrarConexion() {

        enviarEstado("Cerrando socket (" + Thread.currentThread().stackTrace.getOrNull(3)?.methodName + ")")

        try {

            outputStream?.flush()

        } catch (_: Exception) {
        }

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

    private fun detenerTransmision() {

        if (!running && mediaProjection == null && encoder == null && virtualDisplay == null) {
            activo = false
            enviarEvento(TIPO_TERMINADA, "Transmision terminada")
            stopSelf()
            return
        }

        enviarEstado(
            "DETENIENDO CAPTURA..."
        )

        running = false

        transmisionPausada = false

        cerrarConexion()

        try {

            virtualDisplay?.release()

        } catch (_: Exception) {
        }

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

        try {

            inputSurface?.release()

        } catch (_: Exception) {
        }

        inputSurface = null

        try {

            mediaProjection?.unregisterCallback(
                mediaProjectionCallback
            )

        } catch (_: Exception) {
        }

        try {

            mediaProjection?.stop()

        } catch (_: Exception) {
        }

        mediaProjection = null

        activo = false
        enviarEvento(TIPO_TERMINADA, "Transmision terminada")

        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun enviarEvento(tipo: String, mensaje: String) {
        if (tipo != TIPO_LOG) {
            ultimoTipo = tipo
            ultimoMensaje = mensaje
        }
        val intent = Intent(ACTION_ESTADO)
        intent.setPackage(packageName)
        intent.putExtra(EXTRA_TIPO, tipo)
        intent.putExtra(EXTRA_MENSAJE, mensaje)
        sendBroadcast(intent)
    }
    private fun enviarEstado(mensaje: String) {
        enviarEvento(TIPO_LOG, mensaje)
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
        val estado = if (transmisionPausada) "Transmision en pausa" else "Transmitiendo pantalla"

        val accionPausa = Notification.Action.Builder(
            Icon.createWithResource(
                this,
                if (transmisionPausada) {
                    android.R.drawable.ic_media_play
                } else {
                    android.R.drawable.ic_media_pause
                },
            ),
            if (transmisionPausada) "Reanudar" else "Pausar",
            intentDifundido(ACTION_PAUSAR, 10, !transmisionPausada),
        ).build()

        val accionTerminar = Notification.Action.Builder(
            Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel),
            "Finalizar",
            intentDifundido(ACTION_TERMINAR, 11, null),
        ).build()

        val notificacion = Notification.Builder(this, "transfel_capture")
            .setContentTitle("TransFEL")
            .setContentText(estado)
            .setSmallIcon(android.R.drawable.ic_menu_view)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setOngoing(true)
            .setShowWhen(false)
            .setAutoCancel(false)
            .addAction(accionPausa)
            .addAction(accionTerminar)
            .build()

        // FLAG_NO_CLEAR impide que se descarte al deslizar o con "borrar todo".
        notificacion.flags = notificacion.flags or
            Notification.FLAG_NO_CLEAR or
            Notification.FLAG_ONGOING_EVENT or
            Notification.FLAG_FOREGROUND_SERVICE

        return notificacion
    }

    /** PendingIntent que dispara uno de nuestros broadcasts internos. */
    private fun intentDifundido(accion: String, codigo: Int, pausada: Boolean?): PendingIntent {
        val intent = Intent(accion).apply {
            setPackage(packageName)
            if (pausada != null) putExtra("pausada", pausada)
        }

        return PendingIntent.getBroadcast(
            this,
            codigo,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    /** Redibuja la notificacion para que los botones reflejen el estado. */
    private fun actualizarNotificacion() {
        runCatching {
            val manager = getSystemService(NotificationManager::class.java)
            manager.notify(1, createNotification())
        }
    }

    override fun onDestroy() {
        activo = false

        try {

            unregisterReceiver(
                pauseReceiver
            )

        } catch (_: Exception) {
        }

        try {

            unregisterReceiver(
                terminarReceiver
            )

        } catch (_: Exception) {
        }

        cerrarConexion()

        try {

            virtualDisplay?.release()

        } catch (_: Exception) {
        }

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

        try {

            inputSurface?.release()

        } catch (_: Exception) {
        }

        inputSurface = null

        try {

            mediaProjection?.unregisterCallback(
                mediaProjectionCallback
            )

        } catch (_: Exception) {
        }

        try {

            mediaProjection?.stop()

        } catch (_: Exception) {
        }

        mediaProjection = null

        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? {

        return null
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        enviarEstado("Config cambiada (orient=${newConfig.orientation})")
        comprobarTamano(0, null)
    }

    /**
     * El giro tarda en asentarse. Exigimos DOS lecturas iguales seguidas antes
     * de darlo por bueno, y un margen de 1s entre rotaciones aplicadas, para
     * que lecturas intermedias no provoquen un ida y vuelta infinito.
     */
    private fun comprobarTamano(intento: Int, anterior: Pair<Int, Int>?) {
        if (intento > 10) return

        android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
            if (!running || anchoActual == 0) return@postDelayed

            val ahora = tamanoPantalla()

            when {
                // Lectura invalida: reintentar.
                ahora == null -> comprobarTamano(intento + 1, null)

                // Sin cambios respecto a lo que ya tenemos.
                ahora.first == anchoActual && ahora.second == altoActual ->
                    comprobarTamano(intento + 1, ahora)

                // Cambio detectado pero aun no confirmado por una segunda lectura.
                anterior != ahora -> comprobarTamano(intento + 1, ahora)

                // Confirmado. Respetamos el margen entre rotaciones.
                System.currentTimeMillis() - ultimaRotacion < 1000 -> {}

                else -> {
                    ultimaRotacion = System.currentTimeMillis()
                    Thread { actualizarOrientacion(ahora.first, ahora.second) }.start()
                }
            }
        }, 150L)
    }
}
