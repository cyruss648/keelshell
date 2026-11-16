$ps=@(Get-Process|Sort-Object Id);if($ps.Count-gt 20000){throw 'invalid'}
$rows=@(foreach($p in $ps){$w=$null;try{$w=U $p.WorkingSet64}catch{};@{pid=(U $p.Id);name=$p.ProcessName;working_set_bytes=$w}})
F 'ok' @{count=$rows.Count;processes=@($rows)} 0
