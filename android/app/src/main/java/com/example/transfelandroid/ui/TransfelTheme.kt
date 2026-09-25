package com.example.transfelandroid.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Misma paleta que el escritorio (theme.rs)
val Fondo       = Color(0xFF0F0F12)
val Panel       = Color(0xFF16161B)
val Tarjeta     = Color(0xFF1C1C22)
val TarjetaAlta = Color(0xFF26262E)
val Borde       = Color(0xFF373741)
val Acento      = Color(0xFF5A82FF)
val Exito       = Color(0xFF46C878)
val Peligro     = Color(0xFFE65A5A)
val Apagado     = Color(0xFF8C8C96)
val Advertencia = Color(0xFFE0A33C)

private val EsquemaTransfel = darkColorScheme(
    primary = Acento,
    onPrimary = Color.White,
    secondary = TarjetaAlta,
    onSecondary = Color.White,
    background = Fondo,
    onBackground = Color(0xFFECECF1),
    surface = Panel,
    onSurface = Color(0xFFECECF1),
    surfaceVariant = Tarjeta,
    onSurfaceVariant = Apagado,
    error = Peligro,
    onError = Color.White,
    outline = Borde,
)

@Composable
fun TransfelTheme(
    @Suppress("UNUSED_PARAMETER") oscuro: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    // TransFEL es siempre oscuro, igual que el escritorio.
    MaterialTheme(colorScheme = EsquemaTransfel, content = content)
}