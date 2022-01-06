.zeropage

.export FillScreenPage
FillScreenPage: .res 2

; ==============================================================================

.code

; ------------------------------------------------------------------------------

; Fills a 1KiB area with a byte stored in the accumulator. The start address
; should be stored at FillScreenPage. The procedure clobbers X and Y registers,
; as well as FillScreenPage.
.export FillScreen
.proc FillScreen
            ldx #4                      ; 4 pages = 1KiB
PageLoop:   ldy #0
Loop:       sta (FillScreenPage),y      ; Fill one page
            iny
            bne Loop
            inc FillScreenPage+1        ; Next page
            dex
            bne PageLoop
            rts
.endproc
