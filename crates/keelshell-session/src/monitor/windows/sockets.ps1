$ts=@(Get-NetTCPConnection -State Listen);$us=@(Get-NetUDPEndpoint)
if($ts.Count+$us.Count-gt 20000){throw 'invalid'}
$rows=@(foreach($s in $ts){@{protocol='tcp';address=$s.LocalAddress;port=(U $s.LocalPort);pid=(U $s.OwningProcess)}};foreach($s in $us){@{protocol='udp';address=$s.LocalAddress;port=(U $s.LocalPort);pid=(U $s.OwningProcess)}})
F 'ok' @{count=$rows.Count;sockets=@($rows)} 0
