/// A native result-history entry, not an active connection or a delivery receipt.
class ConnectionObservation {
  ConnectionObservation.decode(Map<String, Object?> data)
    : this._(
        data,
        data['flow'] as Map<String, Object?>,
        data['decision'] as Map<String, Object?>?,
      );

  ConnectionObservation._(
    Map<String, Object?> data,
    Map<String, Object?> flow,
    Map<String, Object?>? decision,
  ) : host = flow['host'] as String,
      addresses = List.unmodifiable(
        (flow['ips'] as List<Object?>).cast<String>(),
      ),
      port = flow['port'] as int,
      network = flow['network'] as String,
      inbound = flow['inbound'] as String,
      domainCandidates = List.unmodifiable(
        (flow['domain_candidates'] as List<Object?>? ?? const []).cast<String>(),
      ),
      process = flow['process'] == null
          ? null
          : ConnectionProcessObservation.decode(
              flow['process'] as Map<String, Object?>,
            ),
      hasDecision = decision != null,
      version = (decision?['version'] ?? data['version']) as int?,
      ruleId = decision?['rule_id'] as String?,
      source = decision?['source'] as String?,
      policy = decision?['policy'] as String?,
      expectedAction = decision?['action'] as String?,
      reason = decision?['reason'] as String?,
      outbound = data['outbound'] as String?,
      error = data['error'] as String?,
      message = data['message'] as String?,
      dns = List.unmodifiable(
        (data['dns'] as List<Object?>).map(
          (item) =>
              ConnectionDnsObservation.decode(item as Map<String, Object?>),
        ),
      );

  final String host;
  final List<String> addresses;
  final int port;
  final String network;
  final String inbound;
  final List<String> domainCandidates;
  final ConnectionProcessObservation? process;
  final bool hasDecision;
  final int? version;
  final String? ruleId;
  final String? source;
  final String? policy;
  final String? expectedAction;
  final String? reason;
  final String? outbound;
  final String? error;
  final String? message;
  final List<ConnectionDnsObservation> dns;
}

class ConnectionProcessObservation {
  ConnectionProcessObservation.decode(Map<String, Object?> data)
    : pid = data['pid'] as int?,
      name = data['name'] as String?,
      path = data['path'] as String?,
      source = data['source'] as String?,
      confidence = data['confidence'] as String?;

  final int? pid;
  final String? name;
  final String? path;
  final String? source;
  final String? confidence;
}

class ConnectionDnsObservation {
  ConnectionDnsObservation.decode(Map<String, Object?> data)
    : host = data['host'] as String,
      addresses = List.unmodifiable(
        (data['addresses'] as List<Object?>).cast<String>(),
      ),
      source = (data['source'] as Map<String, Object?>)['kind'] as String,
      server = (data['source'] as Map<String, Object?>)['server'] as String?,
      cached = data['cached'] as bool,
      ttlSeconds = data['ttl_seconds'] as int?,
      warnings = List.unmodifiable(
        (data['warnings'] as List<Object?>).cast<String>(),
      );

  final String host;
  final List<String> addresses;
  final String source;
  final String? server;
  final bool cached;
  final int? ttlSeconds;
  final List<String> warnings;
}
