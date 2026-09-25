package com.example.transfelandroid

import android.Manifest
import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import com.example.transfelandroid.ui.*

class MainActivity : ComponentActivity() {

    private lateinit var projectionManager: MediaProjectionManager

    private var estado by mutableStateOf(EstadoTransmision.INACTIVA)
    private var mensaje by mutableStateOf("Listo para transmitir")
    private var pausada by mutableStateOf(false)

    private val registro: SnapshotStateList<String> = mutableStateListOf()

    // ── Recepción de eventos del servicio ──

    private val estadoReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action != ScreenCaptureService.ACTION_ESTADO) return

            val tipo = intent.getStringExtra(ScreenCaptureService.EXTRA_TIPO)
                ?: ScreenCaptureService.TIPO_LOG
            val texto = intent.getStringExtra(ScreenCaptureService.EXTRA_MENSAJE).orEmpty()

            anotar(texto)

            when (tipo) {
                ScreenCaptureService.TIPO_CONECTANDO -> {
                    estado = EstadoTransmision.CONECTANDO
                    mensaje = texto
                }
                ScreenCaptureService.TIPO_TRANSMITIENDO -> {
                    estado = EstadoTransmision.TRANSMITIENDO
                    pausada = false
                    mensaje = "Transmitiendo al PC"
                }
                ScreenCaptureService.TIPO_PAUSADA -> {
                    estado = EstadoTransmision.PAUSADA
                    pausada = true
                    mensaje = "Transmision en pausa"
                }
                ScreenCaptureService.TIPO_TERMINADA -> {
                    estado = EstadoTransmision.INACTIVA
                    pausada = false
                    mensaje = "Listo para transmitir"
                }
                ScreenCaptureService.TIPO_ERROR -> {
                    estado = EstadoTransmision.ERROR
                    pausada = false
                    mensaje = texto
                }
            }
        }
    }

    // ── Permisos y captura ──

    private val permisoNotificaciones =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { concedido ->
            if (!concedido) {
                anotar("Sin permiso de notificaciones: la captura puede detenerse en segundo plano")
            }
        }

    private val capturaLauncher =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            val data = result.data

            when {
                result.resultCode != Activity.RESULT_OK -> {
                    estado = EstadoTransmision.INACTIVA
                    mensaje = "Permiso cancelado"
                    anotar("El usuario cancelo la captura")
                }
                data == null -> {
                    estado = EstadoTransmision.ERROR
                    mensaje = "Android no devolvio los datos de captura"
                }
                else -> {
                    estado = EstadoTransmision.CONECTANDO
                    mensaje = "Iniciando servicio..."
                    iniciarServicio(result.resultCode, data)
                }
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        projectionManager =
            getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager

        ContextCompat.registerReceiver(
            this,
            estadoReceiver,
            IntentFilter(ScreenCaptureService.ACTION_ESTADO),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )

        pedirNotificaciones()

        setContent {
            TransfelTheme {
                PantallaPrincipal(
                    estado = estado,
                    mensaje = mensaje,
                    pausada = pausada,
                    registro = registro,
                    onIniciar = ::solicitarCaptura,
                    onPausar = ::alternarPausa,
                    onTerminar = ::terminar,
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // El servicio pudo morir mientras la app estaba en segundo plano.
        if (!ScreenCaptureService.activo && estado.enCurso) {
            estado = EstadoTransmision.INACTIVA
            pausada = false
            mensaje = "La transmision se detuvo"
        }
    }

    override fun onDestroy() {
        runCatching { unregisterReceiver(estadoReceiver) }
        super.onDestroy()
    }

    // ── Acciones, todas validadas ──

    private fun solicitarCaptura() {
        if (!estado.puedeIniciar) {
            anotar("Ya hay una transmision en curso")
            return
        }
        estado = EstadoTransmision.PREPARANDO
        mensaje = "Esperando tu permiso..."
        pausada = false
        runCatching { capturaLauncher.launch(projectionManager.createScreenCaptureIntent()) }
            .onFailure {
                estado = EstadoTransmision.ERROR
                mensaje = "Este dispositivo no permite capturar la pantalla"
            }
    }

    private fun alternarPausa() {
        if (!estado.puedePausar) {
            anotar("No hay transmision que pausar")
            return
        }

        val nuevo = !pausada
        enviarOrden(ScreenCaptureService.ACTION_PAUSAR) { putExtra("pausada", nuevo) }
    }

    private fun terminar() {
        if (!estado.puedeTerminar) {
            anotar("No hay transmision que terminar")
            return
        }
        mensaje = "Deteniendo..."
        enviarOrden(ScreenCaptureService.ACTION_TERMINAR)
    }

    private fun enviarOrden(accion: String, extras: Intent.() -> Unit = {}) {
        val intent = Intent(accion).apply {
            setPackage(packageName)
            extras()
        }
        sendBroadcast(intent)
    }

    private fun iniciarServicio(resultCode: Int, data: Intent) {
        val serviceIntent = Intent(this, ScreenCaptureService::class.java).apply {
            putExtra("resultCode", resultCode)
            putExtra("data", data)
        }

        runCatching { ContextCompat.startForegroundService(this, serviceIntent) }
            .onFailure {
                estado = EstadoTransmision.ERROR
                mensaje = "No se pudo iniciar el servicio: ${it.message}"
            }
    }

    private fun pedirNotificaciones() {
        if (Build.VERSION.SDK_INT < 33) return
        val concedido = ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.POST_NOTIFICATIONS,
        ) == PackageManager.PERMISSION_GRANTED

        if (!concedido) permisoNotificaciones.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun anotar(texto: String) {
        if (texto.isBlank()) return
        registro.add(texto)
        if (registro.size > 60) registro.removeAt(0)
    }
}

// ═══════════════════════════ UI ═══════════════════════════

@Composable
private fun PantallaPrincipal(
    estado: EstadoTransmision,
    mensaje: String,
    pausada: Boolean,
    registro: List<String>,
    onIniciar: () -> Unit,
    onPausar: () -> Unit,
    onTerminar: () -> Unit,
) {
    var mostrarRegistro by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Fondo)
            .padding(horizontal = 20.dp)
            .padding(top = 32.dp, bottom = 24.dp),
    ) {
        Encabezado()

        Spacer(Modifier.height(28.dp))

        TarjetaEstado(estado = estado, mensaje = mensaje)

        Spacer(Modifier.height(24.dp))

        BotonPrincipal(estado = estado, onIniciar = onIniciar)

        Spacer(Modifier.height(12.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            BotonSecundario(
                texto = if (pausada) "Reanudar" else "Pausar",
                habilitado = estado.puedePausar,
                color = if (pausada) Exito else Advertencia,
                modifier = Modifier.weight(1f),
                onClick = onPausar,
            )

            BotonSecundario(
                texto = "Terminar",
                habilitado = estado.puedeTerminar,
                color = Peligro,
                modifier = Modifier.weight(1f),
                onClick = onTerminar,
            )
        }

        if (!estado.enCurso && estado != EstadoTransmision.INACTIVA) {
            Spacer(Modifier.height(10.dp))
            Text(
                text = "Pausar y terminar se activan cuando la transmision este en marcha.",
                color = Apagado,
                fontSize = 12.sp,
            )
        }

        Spacer(Modifier.height(24.dp))

        Ayuda(estado)

        Spacer(Modifier.weight(1f))

        TextButton(onClick = { mostrarRegistro = !mostrarRegistro }) {
            Text(
                text = if (mostrarRegistro) "Ocultar detalles ▲" else "Ver detalles tecnicos ▼",
                color = Apagado,
                fontSize = 13.sp,
            )
        }

        AnimatedVisibility(visible = mostrarRegistro) {
            Registro(registro)
        }
    }
}

@Composable
private fun Encabezado() {
    Column {
        Text(
            text = "TransFEL",
            color = Color.White,
            fontSize = 30.sp,
            fontWeight = FontWeight.Bold,
        )
        Text(
            text = "Transmite la pantalla a tu PC",
            color = Apagado,
            fontSize = 14.sp,
        )
    }
}

@Composable
private fun TarjetaEstado(estado: EstadoTransmision, mensaje: String) {
    val (color, titulo) = when (estado) {
        EstadoTransmision.INACTIVA -> Apagado to "Inactiva"
        EstadoTransmision.PREPARANDO -> Acento to "Preparando"
        EstadoTransmision.CONECTANDO -> Acento to "Conectando"
        EstadoTransmision.TRANSMITIENDO -> Exito to "En vivo"
        EstadoTransmision.PAUSADA -> Advertencia to "En pausa"
        EstadoTransmision.ERROR -> Peligro to "Error"
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(Tarjeta)
            .border(1.dp, if (estado.enCurso) color else Borde, RoundedCornerShape(14.dp))
            .padding(18.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Punto(color = color, animado = estado == EstadoTransmision.TRANSMITIENDO)
            Spacer(Modifier.width(10.dp))
            Text(
                text = titulo,
                color = color,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
            )
            if (estado.ocupada) {
                Spacer(Modifier.width(12.dp))
                CircularProgressIndicator(
                    modifier = Modifier.size(16.dp),
                    strokeWidth = 2.dp,
                    color = Acento,
                )
            }
        }

        Spacer(Modifier.height(8.dp))

        Text(text = mensaje, color = Apagado, fontSize = 14.sp)
    }
}

@Composable
private fun Punto(color: Color, animado: Boolean) {
    val transicion = rememberInfiniteTransition(label = "punto")
    val opacidad by transicion.animateFloat(
        initialValue = 1f,
        targetValue = 0.25f,
        animationSpec = infiniteRepeatable(tween(900), RepeatMode.Reverse),
        label = "opacidad",
    )

    Box(
        modifier = Modifier
            .size(11.dp)
            .alpha(if (animado) opacidad else 1f)
            .clip(CircleShape)
            .background(color),
    )
}

@Composable
private fun BotonPrincipal(estado: EstadoTransmision, onIniciar: () -> Unit) {
    val texto = when {
        estado.enCurso -> "Transmision activa"
        estado.ocupada -> "Espera un momento..."
        estado == EstadoTransmision.ERROR -> "Reintentar"
        else -> "Iniciar transmision"
    }

    Button(
        onClick = onIniciar,
        enabled = estado.puedeIniciar,
        modifier = Modifier
            .fillMaxWidth()
            .height(54.dp),
        shape = RoundedCornerShape(12.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = Acento,
            contentColor = Color.White,
            disabledContainerColor = TarjetaAlta,
            disabledContentColor = Apagado,
        ),
    ) {
        Text(text = texto, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
private fun BotonSecundario(
    texto: String,
    habilitado: Boolean,
    color: Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    OutlinedButton(
        onClick = onClick,
        enabled = habilitado,
        modifier = modifier.height(46.dp),
        shape = RoundedCornerShape(11.dp),
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = Tarjeta,
            contentColor = color,
            disabledContainerColor = Tarjeta,
            disabledContentColor = Borde,
        ),
        border = androidx.compose.foundation.BorderStroke(
            1.dp,
            if (habilitado) color.copy(alpha = 0.55f) else Borde,
        ),
    ) {
        Text(text = texto, fontSize = 14.sp, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun Ayuda(estado: EstadoTransmision) {
    val texto = when (estado) {
        EstadoTransmision.INACTIVA ->
            "1. Abre TransFEL en el PC.\n" +
                    "2. Conecta el cable USB o vincula por WiFi.\n" +
                    "3. Pulsa Iniciar transmision y acepta el permiso."
        EstadoTransmision.ERROR ->
            "Revisa que TransFEL este abierto en el PC y que el dispositivo siga conectado."
        else -> "Puedes salir de la app: la transmision sigue en segundo plano."
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(Panel)
            .padding(16.dp),
    ) {
        Text(
            text = if (estado == EstadoTransmision.ERROR) "QUE REVISAR" else "COMO FUNCIONA",
            color = Apagado,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
        )
        Spacer(Modifier.height(8.dp))
        Text(text = texto, color = Color(0xFFBFBFC9), fontSize = 13.sp, lineHeight = 20.sp)
    }
}

@Composable
private fun Registro(registro: List<String>) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(max = 190.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(Color(0xFF0A0A0C))
            .border(1.dp, Borde, RoundedCornerShape(10.dp))
            .padding(12.dp)
            .verticalScroll(rememberScrollState()),
    ) {
        if (registro.isEmpty()) {
            Text("Sin actividad.", color = Apagado, fontSize = 12.sp)
        } else {
            registro.takeLast(40).forEach { linea ->
                Text(
                    text = linea,
                    color = Apagado,
                    fontSize = 11.sp,
                    fontFamily = FontFamily.Monospace,
                    lineHeight = 16.sp,
                )
            }
        }
    }
}