# Real key presses into whatever has the keyboard: the spike's --demo checks where focus went.
param([string]$text)
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait($text)
