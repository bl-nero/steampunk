.macpack cbm                            ; for scrcode macro
.include "c64.inc"

SCREEN_START   = $0400
COLOR_START    = $D800
COL_WHITE      = 1
COL_BLUE       = 6
COL_LIGHT_BLUE = 14

.zeropage

FillScreenPage: .res 2

.code

Reset:      lda #COL_BLUE
            sta VIC_BG_COLOR0
            lda #COL_LIGHT_BLUE
            sta VIC_BORDERCOLOR

            lda #>SCREEN_START
            sta FillScreenPage+1
            lda #<SCREEN_START
            sta FillScreenPage
            lda #32                     ; space screen code
            jsr FillScreen

            ldx #(HelloEnd - Hello)
Loop:       lda Hello-1, x
            sta SCREEN_START-1, x
            lda #COL_LIGHT_BLUE
            sta COLOR_START-1, x
            dex
            bne Loop
            lda #COL_WHITE
            sta COLOR_START + HelloEnd - Hello - 1
End:        jmp End


; Fills a 1KiB area with a byte stored in the accumulator. The start address
; should be stored at FillScreenPage. The procedure clobbers X and Y registers,
; as well as FillScreenPage.
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


.rodata

Hello:      scrcode "hello, world!"
HelloEnd:


.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ