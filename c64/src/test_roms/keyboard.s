; ==============================================================================
;
; This program tests the keyboard wiring by displaying a view of the 8x8
; keyboard switch matrix.
;
; ==============================================================================

.macpack cbm                            ; for scrcode macro
.include "c64.inc"

SCREEN_START   = $0400
COLOR_START    = $D800
COL_WHITE      = 1
COL_BLUE       = 6
COL_LIGHT_BLUE = 14
COL_LIGHT_GREY = 15

CIRCLE_EMPTY = $57
CIRCLE_FULL  = $51

; ==============================================================================

.zeropage

.import FillScreenPage

ScanResult: .res 8
ResultMask: .res 1

.scope PutCharVars
    LineAddress: .res 2
    Row: .res 1
    Col: .res 1
    Chr: .res 1
.endscope

; ==============================================================================

.code

.import FillScreen

Reset:      sei
            lda #COL_BLUE
            sta VIC_BG_COLOR0
            lda #COL_LIGHT_BLUE
            sta VIC_BORDERCOLOR
            lda #%00001000
            sta VIC_CTRL2

            lda #>SCREEN_START
            sta FillScreenPage+1
            lda #<SCREEN_START
            sta FillScreenPage
            lda #32                     ; space screen code
            jsr FillScreen

            lda #>COLOR_START
            sta FillScreenPage+1
            lda #<COLOR_START
            sta FillScreenPage
            lda #COL_LIGHT_BLUE
            jsr FillScreen

            lda #scrbyte '0'
            ldy #0
            ldx #2

:           jsr PutChar
            clc
            adc #1
            inx
            cpx #10
            bne :-

            lda #scrbyte '0'
            ldx #0
            ldy #2

:           jsr PutChar
            clc
            adc #1
            iny
            cpy #10
            bne :-

            ; Set up raster IRQ to trigger at line 131 - just after the last
            ; raster of the displayed matrix.
            lda #131
            sta VIC_HLINE
            lda #%00011011
            sta VIC_CTRL1
            lda #%00000001
            sta VIC_IMR

            cli
Wait:       jmp Wait

; ------------------------------------------------------------------------------

.proc Scan
            lda #%00000001              ; Acknowledge VIC IRQ
            sta VIC_IRR
            lda #$FF
            sta CIA1_DDRA
            lda #$00
            sta CIA1_DDRB
            ; inc VIC_BORDERCOLOR

            lda #%11111110
            ldx #0

:           sta CIA1_PRA
            ldy CIA1_PRB
            sty ScanResult,x
            sec
            rol
            inx
            cpx #8
            bne :-

            ldx #0

PrintByte:  lda #%00000001
            sta ResultMask
            ldy #2
PrintBit:   lda ScanResult,x
            and ResultMask
            beq ZeroBit
            lda #CIRCLE_EMPTY
            jmp Continue
ZeroBit:    lda #CIRCLE_FULL
Continue:   inx
            inx
            jsr PutChar
            dex
            dex
            asl ResultMask
            iny
            cpy #10
            bne PrintBit

            inx
            cpx #8
            bne PrintByte

            ; dec VIC_BORDERCOLOR
            rti
.endproc

; ------------------------------------------------------------------------------

.proc PutChar
            sta PutCharVars::Chr
            stx PutCharVars::Col
            sty PutCharVars::Row

            lda PutCharVars::Row
            asl
            tax

            lda ScreenLines,x
            sta PutCharVars::LineAddress
            lda ScreenLines + 1, x
            sta PutCharVars::LineAddress + 1

            lda PutCharVars::Chr
            ldy PutCharVars::Col
            sta (PutCharVars::LineAddress),y

            lda PutCharVars::Chr
            ldx PutCharVars::Col
            ldy PutCharVars::Row
            rts
.endproc


; ==============================================================================

.rodata

ScreenLines:
            .repeat 25, i
                .word SCREEN_START +  i * 40
            .endrep

; ==============================================================================

.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Scan           ; IRQ