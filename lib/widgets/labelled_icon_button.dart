import 'package:flutter/material.dart';

/// Supplies an accessible name while keeping IconButton's actions and focus.
class LabelledIconButton extends StatelessWidget {
  const LabelledIconButton({
    super.key,
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    this.semanticLabel,
  });

  final String tooltip;
  final IconData icon;
  final VoidCallback? onPressed;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) => Tooltip(
    message: tooltip,
    excludeFromSemantics: true,
    child: MergeSemantics(
      child: Semantics(
        label: semanticLabel ?? tooltip,
        child: IconButton(onPressed: onPressed, icon: Icon(icon)),
      ),
    ),
  );
}
