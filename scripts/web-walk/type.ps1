# Real key presses into whatever has the keyboard: the web view walk checks where they went (scripts/web-walk.sh).
param([string]$text)
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait($text)
