package com.example.transfelandroid

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.projection.MediaProjectionManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat

class MainActivity : ComponentActivity() {

    private lateinit var projectionManager: MediaProjectionManager

    private var estado by mutableStateOf(
        "Esperando..."
    )

    private val estadoReceiver =
        object : BroadcastReceiver() {

            override fun onReceive(
                context: Context?,
                intent: Intent?
            ) {

                val mensaje =
                    intent?.getStringExtra(
                        "estado"
                    )

                if (mensaje != null) {

                    estado = mensaje
                }
            }
        }

    private val screenCaptureLauncher =
        registerForActivityResult(
            ActivityResultContracts.StartActivityForResult()
        ) { result ->

            estado = "Resultado recibido"

            if (result.resultCode == Activity.RESULT_OK) {

                estado = "PERMISO ACEPTADO"

                val data = result.data

                if (data != null) {

                    estado = "DATA RECIBIDA"

                    startScreenCapture(
                        result.resultCode,
                        data
                    )

                } else {

                    estado = "ERROR: DATA NULL"
                }

            } else {

                estado = "PERMISO CANCELADO"
            }
        }

    override fun onCreate(
        savedInstanceState: Bundle?
    ) {

        super.onCreate(savedInstanceState)

        projectionManager =
            getSystemService(
                Context.MEDIA_PROJECTION_SERVICE
            ) as MediaProjectionManager

        val filter =
            IntentFilter(
                "com.example.transfelandroid.ESTADO"
            )

        ContextCompat.registerReceiver(
            this,
            estadoReceiver,
            filter,
            ContextCompat.RECEIVER_NOT_EXPORTED
        )

        setContent {

            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(20.dp),

                horizontalAlignment =
                    Alignment.CenterHorizontally,

                verticalArrangement =
                    Arrangement.Center
            ) {

                Text(
                    text = "TransFEL"
                )

                Text(
                    text = estado
                )

                Button(
                    onClick = {

                        estado =
                            "Abriendo captura..."

                        requestScreenCapture()
                    }
                ) {

                    Text(
                        text = "Iniciar captura"
                    )
                }
            }
        }
    }

    private fun requestScreenCapture() {

        estado =
            "Solicitando permiso..."

        val intent =
            projectionManager
                .createScreenCaptureIntent()

        screenCaptureLauncher.launch(intent)
    }

    private fun startScreenCapture(
        resultCode: Int,
        data: Intent
    ) {

        estado =
            "Iniciando servicio..."

        val serviceIntent =
            Intent(
                this,
                ScreenCaptureService::class.java
            )

        serviceIntent.putExtra(
            "resultCode",
            resultCode
        )

        serviceIntent.putExtra(
            "data",
            data
        )

        try {

            ContextCompat.startForegroundService(
                this,
                serviceIntent
            )

            estado =
                "SERVICIO INICIADO"

        } catch (e: Exception) {

            estado =
                "ERROR SERVICE: ${e.message}"
        }
    }

    override fun onDestroy() {

        unregisterReceiver(
            estadoReceiver
        )

        super.onDestroy()
    }
}

