param(
    [ValidateSet('Current', 'All')][string]$Scope = 'Current',
    [ValidatePattern('^S-1-[0-9-]+$')][string]$UserSid,
    [switch]$Utf8Stdout
)
$ErrorActionPreference = 'Stop'
if ($Utf8Stdout) { [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false) }
$snapshotErrors = @()
$query = @{ ErrorAction = 'Stop'; ErrorVariable = '+snapshotErrors' }
if ($Scope -eq 'All') { $query.IncludeAllCompartments = $true }
# Lifetimes, ages and counters are not configuration.
$routes = @(Get-NetRoute @query |
    Select-Object CompartmentId,InterfaceIndex,DestinationPrefix,NextHop,RouteMetric,Protocol,Publish |
    Sort-Object CompartmentId,InterfaceIndex,DestinationPrefix,NextHop,RouteMetric,Protocol,Publish)
$addresses = @(Get-NetIPAddress @query |
    Select-Object CompartmentId,InterfaceIndex,IPAddress,PrefixLength,PrefixOrigin,SuffixOrigin,SkipAsSource |
    Sort-Object CompartmentId,InterfaceIndex,IPAddress)
$interfaces = @(Get-NetIPInterface @query |
    Select-Object CompartmentId,InterfaceIndex,AddressFamily,NlMtu,InterfaceMetric,Dhcp,Forwarding,WeakHostSend,WeakHostReceive |
    Sort-Object CompartmentId,InterfaceIndex,AddressFamily)
$adapters = @(Get-NetAdapter -IncludeHidden -ErrorAction Stop -ErrorVariable +snapshotErrors |
    Select-Object ifIndex,InterfaceGuid,Name,InterfaceDescription,MacAddress,AdminStatus |
    Sort-Object ifIndex,InterfaceGuid)
$dns = @(Get-DnsClientServerAddress -ErrorAction Stop -ErrorVariable +snapshotErrors |
    Select-Object InterfaceIndex,AddressFamily,ServerAddresses |
    Sort-Object InterfaceIndex,AddressFamily)
$proxyPath = if ($UserSid) {
    "Registry::HKEY_USERS\$UserSid\Software\Microsoft\Windows\CurrentVersion\Internet Settings"
} else {
    'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
}
$proxy = Get-ItemProperty -LiteralPath $proxyPath -ErrorAction Stop -ErrorVariable +snapshotErrors
if ($snapshotErrors.Count -gt 0) { throw 'Network snapshot query returned errors; restoration is unproven' }
if ($routes.Count -eq 0 -or $addresses.Count -eq 0 -or $interfaces.Count -eq 0 -or $adapters.Count -eq 0 -or $dns.Count -eq 0) {
    throw 'Network snapshot is incomplete; restoration is unproven'
}
$snapshot = [ordered]@{
    routes = $routes
    addresses = $addresses
    interfaces = $interfaces
    adapters = $adapters
    dns = $dns
    proxy = [ordered]@{
        ProxyEnable = $proxy.ProxyEnable
        ProxyServer = $proxy.ProxyServer
        ProxyOverride = $proxy.ProxyOverride
        AutoConfigURL = $proxy.AutoConfigURL
    }
} | ConvertTo-Json -Depth 12
if ($Utf8Stdout) {
    [Console]::WriteLine($snapshot)
    [Console]::Out.Flush()
} else {
    $snapshot
}
