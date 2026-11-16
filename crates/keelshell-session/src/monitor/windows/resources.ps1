$o=@(Get-CimInstance Win32_OperatingSystem);if($o.Count-ne 1){throw 'invalid'};$o=$o[0]
if($null-eq$o.TotalVisibleMemorySize-or$null-eq$o.FreePhysicalMemory){throw 'invalid'}
$b=[datetime]$o.LastBootUpTime;$u=([datetime]$o.LocalDateTime-$b).TotalSeconds
$p=@(Get-CimInstance Win32_Processor);if($p.Count-gt 256){throw 'invalid'};$c=$null;$w=0D;$s=0D;$k=0
foreach($a in $p){if($null-ne$a.LoadPercentage){$l=[decimal]$a.LoadPercentage;$n=[decimal]$a.NumberOfLogicalProcessors;if($l-lt 0-or$l-gt 100-or$n-lt 1-or$n%1){throw 'invalid'};$s+=$l*$n;$w+=$n;$k++}}
if($k){if($k-ne$p.Count){throw 'invalid'};$c=[double]($s/$w)}
$ns=@(foreach($a in [Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces()){$t=$a.GetIPStatistics();@{name=$a.Name;received_bytes=(U $t.BytesReceived);transmitted_bytes=(U $t.BytesSent)}});if($ns.Count-gt 256){throw 'invalid'}
$f=@();$cs='skipped';if(!$skip){try{$f=@(foreach($a in [IO.DriveInfo]::GetDrives()){if($a.DriveType-eq'Fixed'-and$a.IsReady){@{source=$a.Name;mount=$a.RootDirectory.FullName;total_bytes=(U $a.TotalSize);total_free_bytes=(U $a.TotalFreeSpace);available_bytes=(U $a.AvailableFreeSpace)}}});if($f.Count-gt 512){throw 'invalid'};$cs='ok'}catch{$f=@();$cs=E $_}}
F 'ok' @{boot_time=(U ([math]::Floor(([decimal]$b.ToUniversalTime().Ticks-621355968000000000)/10000000)));uptime_seconds=$u;cpu_percent=$c;memory_total_bytes=(U ([decimal]$o.TotalVisibleMemorySize*1024));memory_free_bytes=(U ([decimal]$o.FreePhysicalMemory*1024));network_count=$ns.Count;networks=@($ns);capacity_status=$cs;filesystem_count=$f.Count;filesystems=@($f)} 0
