$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';$WarningPreference='SilentlyContinue';$v=$PSVersionTable.PSVersion
function E($r){for($e=$r.Exception;$e;$e=$e.InnerException){if($e-is[Management.Automation.CommandNotFoundException]){return 'command_missing'};if($e-is[Management.Automation.ParameterBindingException]-or$e-is[NotSupportedException]){return 'unsupported_options'};if($e-is[UnauthorizedAccessException]-or$e-is[Security.SecurityException]-or$e.HResult-eq-2147024891){return 'permission_denied'}};if($r.CategoryInfo.Category-eq'PermissionDenied'){return 'permission_denied'};'command_failed'}
function U($n){if($null-eq$n-or[decimal]$n-lt 0-or[decimal]$n%1){throw 'invalid'};([uint64]$n).ToString([cultureinfo]::InvariantCulture)}
function F($s,$d,$x){$j=@{version=1;operation=$op;platform='Windows';runtime='WindowsPowerShell';runtime_version=$v.ToString();units='bytes seconds percent';status=$s;data=$d}|ConvertTo-Json -Depth 5 -Compress;if([Text.Encoding]::UTF8.GetByteCount($j)-gt 2097150){F 'command_failed' $null 1};[Console]::WriteLine($j);exit $x}
try{
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
if($PSVersionTable.PSEdition-ne'Desktop'-or$v.Major-ne 5-or$v.Minor-ne 1-or[Environment]::OSVersion.Platform-ne'Win32NT'){throw [NotSupportedException]::new()}
