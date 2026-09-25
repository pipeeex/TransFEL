package com.example.transfelandroid

enum class EstadoTransmision {
    INACTIVA,
    PREPARANDO,
    CONECTANDO,
    TRANSMITIENDO,
    PAUSADA,
    ERROR;

    /** Solo se puede pausar o terminar cuando hay algo corriendo. */
    val enCurso: Boolean
        get() = this == TRANSMITIENDO || this == PAUSADA

    val puedeIniciar: Boolean
        get() = this == INACTIVA || this == ERROR

    val puedePausar: Boolean
        get() = enCurso

    val puedeTerminar: Boolean
        get() = this != INACTIVA && this != ERROR

    val ocupada: Boolean
        get() = this == PREPARANDO || this == CONECTANDO
}