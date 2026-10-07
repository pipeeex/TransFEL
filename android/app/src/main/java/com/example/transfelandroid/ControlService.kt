package com.example.transfelandroid

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.graphics.PointF
import android.os.Handler
import android.os.Looper
import android.view.accessibility.AccessibilityEvent
import kotlin.math.abs
import kotlin.math.hypot

/**
 * Reproduce en el telefono los gestos que llegan desde el PC.
 *
 * Una app normal no puede inyectar eventos en otras apps; un
 * AccessibilityService si, mediante dispatchGesture(). El usuario tiene que
 * activarlo una vez en Ajustes de Accesibilidad.
 */
class ControlService : AccessibilityService() {

    companion object {
        /** Null mientras el servicio este desactivado en Ajustes. */
        @Volatile
        var instancia: ControlService? = null
            private set

        val activo: Boolean
            get() = instancia != null

        private const val PASO_MS = 24L            // duracion de cada tramo
        private const val UMBRAL_ARRASTRE = 24f    // px
        private const val PULSACION_LARGA = 450L   // ms
        private const val DURACION_MAXIMA = 2000L  // ms
        private const val RECORRIDO_MINIMO = 40f   // px, por debajo seria un toque
    }

    private val hilo = Handler(Looper.getMainLooper())

    // Gesto en curso
    private var cierrePendiente: PointF? = null
    private var trazoActual: GestureDescription.StrokeDescription? = null
    private var pendiente: PointF? = null
    private var despachando = false
    private var continuoRoto = false   // si continueStroke falla, modo simple
    private var largaEnviada = false


    private var inicioX = 0f
    private var inicioY = 0f
    private var inicioMs = 0L
    private var ultimoX = 0f
    private var ultimoY = 0f
    private var enGesto = false

    private val tareaLarga = Runnable {
        // Sigue el dedo abajo, quieto y sin haberse vuelto arrastre.
        if (enGesto && trazoActual == null && !largaEnviada) {
            largaEnviada = true
            gesto(inicioX, inicioY, inicioX, inicioY, 600L)
        }
    }
    private val resultado = object : GestureResultCallback() {
        override fun onCompleted(gesto: GestureDescription?) {
            despachando = false

            val cierre = cierrePendiente
            if (cierre  != null){
                cierrePendiente = null
                cerrarTrazo(cierre.x, cierre.y)
                return
            }
            continuar()
        }

        override fun onCancelled(gesto: GestureDescription?) {
            despachando = false
            trazoActual = null
            pendiente = null
            cierrePendiente = null
        }
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instancia = this
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}

    override fun onInterrupt() {}

    override fun onDestroy() {
        instancia = null
        super.onDestroy()
    }

    // ── Entrada desde el PC ──────────────────────────────────────────

    fun abajo(x: Float, y: Float) {
        cancelar()

        inicioX = x
        inicioY = y
        ultimoX = x
        ultimoY = y
        inicioMs = System.currentTimeMillis()
        enGesto = true
        largaEnviada = false
        hilo.removeCallbacks(tareaLarga)
        hilo.postDelayed(tareaLarga, PULSACION_LARGA)
    }

    fun mover(x: Float, y: Float) {
        if (!enGesto) return

        if (continuoRoto) {
            // Modo simple: solo se recuerda, se resuelve al soltar.
            ultimoX = x
            ultimoY = y
            return
        }

        if (trazoActual == null) {
            // Aun no es arrastre: esperamos a superar el umbral.
            if (hypot(x - inicioX, y - inicioY) < UMBRAL_ARRASTRE) {
                ultimoX = x
                ultimoY = y
                return
            }
            // Ya es arrastre: la pulsacion larga queda descartada.
            hilo.removeCallbacks(tareaLarga)
            abrirTrazo(x, y)
            return
        }

        // Solo se guarda la ultima posicion: si llegan diez movimientos
        // mientras hay un gesto en vuelo, los nueve primeros sobran.
        pendiente = PointF(x, y)
        if (!despachando) continuar()
    }

    fun arriba(x: Float, y: Float) {
        hilo.removeCallbacks(tareaLarga)

        if (!enGesto) {
            toque(x, y)
            return
        }
        enGesto = false

        if (largaEnviada) {
            // La pulsacion larga ya se reprodujo mientras mantenias.
            largaEnviada = false
            trazoActual = null
            pendiente = null
            return
        }
        if (trazoActual != null) {
            if (despachando) {
                // Se cierra en cuanto termine el tramo en vuelo.
                cierrePendiente = PointF(x, y)
            } else {
                cerrarTrazo(x, y)
            }
            return
        }

        // Nunca llego a ser arrastre: toque o pulsacion larga.
        val duracion = (System.currentTimeMillis() - inicioMs)
            .coerceIn(1L, DURACION_MAXIMA)

        if (duracion >= PULSACION_LARGA) {
            gesto(inicioX, inicioY, inicioX, inicioY, duracion)
        } else {
            toque(inicioX, inicioY)
        }
    }

    fun cancelar() {
        hilo.removeCallbacks(tareaLarga)
        largaEnviada = false
        enGesto = false
        trazoActual = null
        pendiente = null
        cierrePendiente = null

    }

    fun toque(x: Float, y: Float) {
        gesto(x, y, x, y, 40L)
    }

    fun arrastrar(x1: Float, y1: Float, x2: Float, y2: Float, duracion: Long) {
        gesto(x1, y1, x2, y2, duracion.coerceIn(60L, DURACION_MAXIMA))
    }

    /** Rueda del raton: un deslizamiento vertical rapido. */
    fun rueda(x: Float, y: Float, delta: Float) {
        if (enGesto) return

        // Un recorrido corto Android lo interpreta como toque, no como scroll.
        if (abs(delta) < RECORRIDO_MINIMO) return

        val destino = (y + delta).coerceAtLeast(1f)

        // Rapido: cuanto mas breve, mas se parece a un gesto de desplazamiento.
        gesto(x, y, x, destino, 90L)
    }

    fun accionGlobal(codigo: Int) {
        runCatching { performGlobalAction(codigo) }
    }

    // ── Inyeccion ────────────────────────────────────────────────────

    /** Extiende el trazo abierto hacia la ultima posicion conocida. */
    private fun continuar() {
        val destino = pendiente ?: return
        val anterior = trazoActual ?: return
        pendiente = null

        try {
            val camino = Path().apply {
                moveTo(ultimoX, ultimoY)
                lineTo(
                    if (abs(destino.x - ultimoX) < 0.5f && abs(destino.y - ultimoY) < 0.5f) {
                        destino.x + 0.6f
                    } else {
                        destino.x
                    },
                    destino.y,
                )
            }

            val trazo = anterior.continueStroke(camino, 0L, PASO_MS, true)
            ultimoX = destino.x
            ultimoY = destino.y
            trazoActual = trazo
            despachar(trazo)

        } catch (_: Exception) {
            continuoRoto = true
            trazoActual = null
        }
    }
    private fun abrirTrazo(x: Float, y: Float){
        try {
            val camino = Path().apply {
                moveTo(inicioX, inicioY)
                lineTo(x, y)
            }
            val trazo = GestureDescription.StrokeDescription(camino, 0L, PASO_MS, true)
            ultimoX = x
            ultimoY = y
            trazoActual = trazo
            despachar(trazo)
        }catch (_: Exception){
            continuoRoto = true
            trazoActual = null
        }
    }
    private fun cerrarTrazo(x: Float, y: Float){
        val anterior = trazoActual?: return
        trazoActual = null
        pendiente = null

        try {
            val camino = Path().apply {
                moveTo(ultimoX, ultimoY)
                lineTo(
                    if (abs(x - ultimoX) < 0.5f && abs(y - ultimoY) < 0.5f) x + 0.6f else x,
                    y,
                )
            }
            val cierre = anterior.continueStroke(camino, 0L, PASO_MS, false)
            despachar(cierre)

        } catch (_: Exception) {
            continuoRoto = true
            // Si no se pudo cerrar, al menos se reproduce el arrastre entero.
            arrastrar(inicioX, inicioY, x, y, System.currentTimeMillis() - inicioMs)
        }
    }

    private fun despachar(trazo: GestureDescription.StrokeDescription) {
        try {
            val descripcion = GestureDescription.Builder().addStroke(trazo).build()
            despachando = true
            if (!dispatchGesture(descripcion, resultado, hilo)) {
                despachando = false
            }
        } catch (_: Exception) {
            despachando = false
        }
    }

    private fun gesto(x1: Float, y1: Float, x2: Float, y2: Float, duracion: Long) {
        try {
            val camino = Path().apply {
                moveTo(x1, y1)
                if (abs(x2 - x1) > 0.5f || abs(y2 - y1) > 0.5f) {
                    lineTo(x2, y2)
                } else {
                    lineTo(x1, y1 + 1f)
                }
            }

            val trazo = GestureDescription.StrokeDescription(camino, 0L, duracion)
            val descripcion = GestureDescription.Builder().addStroke(trazo).build()
            dispatchGesture(descripcion, null, null)

        } catch (_: Exception) {
            // Coordenadas fuera de pantalla u otro gesto en curso: se descarta.
        }
    }
}