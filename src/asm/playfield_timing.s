.include "atari2600.inc"

.macro setpf value
            lda #value
            sta PF0
            sta PF1
            sta PF2
.endmacro

Reset:
            lda #0
            tax
:           dex
            sta $00,x
            bne :-
            dex
            txs
            lda #$0E
            sta COLUBK
            lda #$94
            sta COLUPF

StartOfFrame:
            ldy #0

            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Emit 3 scanlines of VSYMC.
            lda #2
            sta VSYNC
            sta WSYNC
            sta WSYNC
            sta WSYNC

            ; Set timer to go off after 31 scanlines (a little bit above 43*64 ticks)
            lda #43
            sta TIM64T


            ; Emit vertical blank until the timer goes off.
            lda #0
            sta VSYNC           
:           lda INTIM
            bpl :-
            sta WSYNC
            lda #0
            sta VBLANK

            ; Emit the test pattern.
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            bit $80
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            bit $80
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            bit $80
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            bit $80
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            bit $80
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            nop
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            nop
            bit $80
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            setpf $00
            sta WSYNC

            setpf $FF
            nop
            nop
            nop
            nop
            nop
            nop
            bit $80
            setpf $00
            sta WSYNC

            ; Wait for the remaining scanlines
            ldx #(192-15)
:           sta WSYNC
            dex
            bne :-

            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Wait for 35*64 cycles, which should be just short of 30 lines of overscan.
            lda #35
            sta TIM64T
:           lda INTIM
            bpl :-
            sta WSYNC

            jmp StartOfFrame

.segment "VECTORS"
            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ