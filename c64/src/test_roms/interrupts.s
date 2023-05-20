; ==============================================================================
;
; This program tests the IRQ wiring by triggering a sequence of interrupts: CIA1
; timer -> CIA2 timer -> VIC raster. Each CIA timer triggers setting up the next
; interrupt in sequence, while the VIC interrupt timer triggers border flashing.
;
; ==============================================================================

.macpack cbm                            ; for scrcode macro
.include "c64.inc"
.include "common.inc"

; Offset to the center of the screen.
TEXT_OFFSET = (25 / 2) * 40 + 20 - (HELLO_LEN + 1) / 2

; ==============================================================================

.zeropage

.import Init
.import FillScreenPage

; ==============================================================================

.code

.import FillScreen

Reset:      jsr Init
            lda #COL_WHITE
            sta VIC_BG_COLOR0
            lda #COL_LIGHT_GREY
            sta VIC_BORDERCOLOR

            lda #>SCREEN_START
            sta FillScreenPage+1
            lda #<SCREEN_START
            sta FillScreenPage
            lda #32                     ; space screen code
            jsr FillScreen

            ldx #HELLO_LEN
Loop:       lda Hello-1, x
            sta SCREEN_START - 1 + TEXT_OFFSET, x
            dex
            bne Loop

            lda #10                     ; Set up CIA1 timer A
            sta CIA1_TA
            lda #0
            sta CIA1_TA + 1
            lda #%10000001              ; CIA1 timer A triggers IRQ
            sta CIA1_ICR
            lda #%00011001              ; Load and start a one-shot trigger
            sta CIA1_CRA

            cli

End:        jmp End

; ------------------------------------------------------------------------------

.proc Irq
            lda CIA1_ICR                ; Poll and acknowledge CIA1 IRQ
            and #%00000001
            bne Cia1Irq                 ; CIA1 IRQ triggered

            lda CIA2_ICR                ; Poll and acknowledge CIA2 IRQ
            and #%00000001
            bne Cia2Irq                 ; CIA2 IRQ triggered

            lda VIC_IRR                 ; Poll VIC IRQ
            and #%00000001
            bne VicIrq                  ; VIC IRQ triggered
            rti

Cia1Irq:    lda #%00000001              ; Turn off CIA1 IRQ
            sta CIA1_ICR
            lda #10                     ; Set up CIA2 timer A
            sta CIA2_TA
            lda #0
            sta CIA2_TA + 1
            lda #%10000001              ; CIA2 timer A triggers IRQ
            sta CIA2_ICR
            lda #%00011001              ; Load and start a one-shot trigger
            sta CIA2_CRA
            rti

Cia2Irq:    lda #%00000001              ; Turn off CIA2 IRQ
            sta CIA2_ICR
            ; Set up raster IRQ interrupt for line 13, which should be the 1st
            ; VBLANK line.
            lda #13
            sta VIC_HLINE
            lda #%00011011
            sta VIC_CTRL1
            lda #%00000001
            sta VIC_IMR
            rti

VicIrq:     lda #%00000001              ; Acknowledge VIC IRQ
            sta VIC_IRR
            inc VIC_BORDERCOLOR         ; Change border color
            rti
.endproc

; ==============================================================================

.rodata

Hello:      scrcode "hello, world!"
HELLO_LEN = * - Hello;

; ==============================================================================

.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Irq            ; IRQ