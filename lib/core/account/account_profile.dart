/// Student identity fields returned by the native account snapshot.
class ShuJwxtProfile {
  const ShuJwxtProfile({
    required this.name,
    this.studentId,
    this.grade,
    this.college,
    this.major,
  });
  final String name;
  final String? studentId;
  final String? grade;
  final String? college;
  final String? major;
}
