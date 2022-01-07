.macpack cbm                            ; for scrcode macro
.include "c64.inc"

SCREEN_START   = $0400
COLOR_START    = $D800
COL_WHITE      = 1
COL_BLUE       = 6
COL_BROWN      = 9
COL_LIGHT_BLUE = 14
COL_LIGHT_GREY = 15

; ==============================================================================

.zeropage

.import FillScreenPage

PrintBcdStart:  .res 2
Counter:        .res 2  ; 4-digit BCD counter

; ==============================================================================

.code

.import FillScreen

; ------------------------------------------------------------------------------

.macro MeasureCia CiaCountdown, CiaControl, OutputAddress
.scope
            ; Measure the CIA timer cycles. They should correspond 1:1 to the
            ; CPU clock.
            ;
            ; Again, start by going to the end of display window to avoid bad
            ; lines.
            lda #251
:           cmp VIC_HLINE
            bne :-

            lda #0                      ; Initialize the counter
            sta Counter
            sta Counter+1
            sed

            lda #<1000                   ; Count down from 1000
            sta CiaCountdown
            lda #>1000
            sta CiaCountdown + 1
            lda #%00011001              ; Load and start a one-shot timer
            sta CiaControl

Measure:    clc                         ; 2 Increase counter by 1
            lda Counter                 ; 3
            adc #1                      ; 2
            sta Counter                 ; 3
            lda Counter+1               ; 3
            adc #0                      ; 2
            sta Counter+1               ; 3
            lda CiaControl              ; 4 Check if still running
            and #%00000001              ; 2
            bne Measure                 ; 3 (2 when leaving)

; Do our best to protect from crossing page boundaries when branching in
; time-critical code.
.assert >Measure = >*, error, "Timer measurement code crosses the page boundary"

            cld                         ; Counting finished

            ; Print the results
            lda #<(OutputAddress)
            sta PrintBcdStart
            lda #>(OutputAddress)
            sta PrintBcdStart + 1
            lda Counter+1
            jsr PrintBcd
            lda Counter
            jsr PrintBcd
.endscope
.endmacro

; ------------------------------------------------------------------------------

Reset:      lda #COL_LIGHT_GREY
            sta VIC_BG_COLOR0
            lda #COL_BROWN
            sta VIC_BORDERCOLOR
            lda #%00001000
            sta VIC_CTRL2

            lda #>SCREEN_START
            sta FillScreenPage+1
            lda #<SCREEN_START
            sta FillScreenPage
            lda #32                     ; space screen code
            jsr FillScreen

            ; Roughly measure the raster line. We don't even attempt to measure
            ; it precisely, since achieving a stable raster interrupt is a
            ; nightmare on C64, but even this should be enough to assert that
            ; the CPU ticks roughly every 8 pixels.
            lda #0                      ; Initialize the counter
            sta Counter
            sta Counter+1
            sed

            ; First, go to the end of display window to avoid bad lines.
            lda #251
:           cmp VIC_HLINE
            bne :-

            ; Since we don't actually know, perhaps we already were in the
            ; middle of raster 251, or at the very end of it, let's wait until
            ; 253, just in case.
            lda #253
:           cmp VIC_HLINE               ; 4
            bne :-                      ; 2 (leaving loop)

            ; Now we are sure we are reasonably close to the beginning of raster
            ; 253. Start measuring until the start of raster 51.
            ldx #51                     ; 2 Raster line 51
MeasureVic: clc                         ; 2 Increase counter by 1
            lda Counter                 ; 3
            adc #1                      ; 2
            sta Counter                 ; 3
            lda Counter+1               ; 3
            adc #0                      ; 2
            sta Counter+1               ; 3
            cpx VIC_HLINE               ; 4
            bne MeasureVic              ; 3 (2 when leaving)

; Do our best to protect from crossing page boundaries when branching in
; time-critical code.
.assert >MeasureVic = >*, error, "Raster measurement code crosses the page boundary"

            cld                         ; Counting finished

            ; Print the results
            lda #<(SCREEN_START + 41)
            sta PrintBcdStart
            lda #>(SCREEN_START + 41)
            sta PrintBcdStart + 1
            lda Counter+1
            jsr PrintBcd
            lda Counter
            jsr PrintBcd

            MeasureCia CIA1_TA, CIA1_CRA, SCREEN_START + 81
            MeasureCia CIA2_TA, CIA2_CRA, SCREEN_START + 121

End:        jmp End

; ------------------------------------------------------------------------------

; Prints a 2-digit, 0-padded BCD number (A) at PrintBcdStart. Clobbers A, X, and
; Y registers. PrintBcdStart will point to the next character cell after the
; number.
.proc PrintBcd
            ldy #0
            tax                         ; Save number for later
            lsr                         ; Take the tens digit
            lsr
            lsr
            lsr
            clc
            adc #'0'                    ; Convert to screen code
            sta (PrintBcdStart),y       ; Store on screen
            inc PrintBcdStart           ; Move one character right
            txa                         ; Restore the saved number
            and #$0F                    ; Take the units digit
            clc
            adc #'0'                    ; Convert to screen code
            sta (PrintBcdStart),y       ; Store on screen
            inc PrintBcdStart           ; Move one character right
            rts
.endproc

; ==============================================================================

.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ
