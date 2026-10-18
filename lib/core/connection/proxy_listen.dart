/// A listener address already validated and normalized by the native owner.
class ShuProxyListen {
  const ShuProxyListen.fromBackend(this.address);
  static const loopback = ShuProxyListen.fromBackend('127.0.0.1');
  static const anyNetwork = ShuProxyListen.fromBackend('0.0.0.0');
  static const anyNetworkV6 = ShuProxyListen.fromBackend('::');
  final String address;
  bool get allInterfaces =>
      address == anyNetwork.address || address == anyNetworkV6.address;
  bool get isLoopback => address == loopback.address;
  bool get exposesToNetwork => !isLoopback;
  @override
  bool operator ==(Object other) =>
      other is ShuProxyListen && other.address == address;
  @override
  int get hashCode => address.hashCode;
  @override
  String toString() => 'ShuProxyListen($address)';
}
