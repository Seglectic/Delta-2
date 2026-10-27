#Requires AutoHotkey v2.0

;  ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
;  │                                                        Discord                                                     │
;  ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
Discord := "Discord"                                                                                        ; Discord window name
DiscordPath := "C:\Users\" . A_UserName . "\AppData\Local\Discord\Update.exe --processStart Discord.exe"    ; Discord path (should be universal on windows)

; Activate the discord window when pressing F19 or pressing Delta + R
F20 & r::
F19::
{
  if WinActive(Discord) {
    WinHide Discord
    return
  } else{
    try{
      WinActivate Discord ; Attempt to activate window (if it's shown but inactive)
    } catch{
      try{
        WinShow Discord ; Otherwise attempt to show window (if it's hidden)
        WinActivate Discord ; And activate it
      } catch{
        ; If all else fails, run DiscordPath and activate it
        RunWait DiscordPath
        WinWaitActive Discord
      }
    }
    return
  }
}

; Rotary Encoder Left for Discord Previous Channel
F16::{
  if WinActive(Discord) {
    Send "!{Up}"
    return
  }	
}

; Rotary Encoder Right for Discord Next Channel
F17::{
  if WinActive(Discord) {
    Send "!{Down}"
    return
  }	
}
