// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'model.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$Block {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Block);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'Block()';
}


}

/// @nodoc
class $BlockCopyWith<$Res>  {
$BlockCopyWith(Block _, $Res Function(Block) __);
}


/// Adds pattern-matching-related methods to [Block].
extension BlockPatterns on Block {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( Block_Text value)?  text,TResult Function( Block_Image value)?  image,TResult Function( Block_Table value)?  table,TResult Function( Block_Attachment value)?  attachment,required TResult orElse(),}){
final _that = this;
switch (_that) {
case Block_Text() when text != null:
return text(_that);case Block_Image() when image != null:
return image(_that);case Block_Table() when table != null:
return table(_that);case Block_Attachment() when attachment != null:
return attachment(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( Block_Text value)  text,required TResult Function( Block_Image value)  image,required TResult Function( Block_Table value)  table,required TResult Function( Block_Attachment value)  attachment,}){
final _that = this;
switch (_that) {
case Block_Text():
return text(_that);case Block_Image():
return image(_that);case Block_Table():
return table(_that);case Block_Attachment():
return attachment(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( Block_Text value)?  text,TResult? Function( Block_Image value)?  image,TResult? Function( Block_Table value)?  table,TResult? Function( Block_Attachment value)?  attachment,}){
final _that = this;
switch (_that) {
case Block_Text() when text != null:
return text(_that);case Block_Image() when image != null:
return image(_that);case Block_Table() when table != null:
return table(_that);case Block_Attachment() when attachment != null:
return attachment(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( List<Inline> inlines)?  text,TResult Function( String url,  String alt)?  image,TResult Function( List<List<List<Inline>>> rows,  bool hasHeaderRow)?  table,TResult Function( String name,  String url)?  attachment,required TResult orElse(),}) {final _that = this;
switch (_that) {
case Block_Text() when text != null:
return text(_that.inlines);case Block_Image() when image != null:
return image(_that.url,_that.alt);case Block_Table() when table != null:
return table(_that.rows,_that.hasHeaderRow);case Block_Attachment() when attachment != null:
return attachment(_that.name,_that.url);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( List<Inline> inlines)  text,required TResult Function( String url,  String alt)  image,required TResult Function( List<List<List<Inline>>> rows,  bool hasHeaderRow)  table,required TResult Function( String name,  String url)  attachment,}) {final _that = this;
switch (_that) {
case Block_Text():
return text(_that.inlines);case Block_Image():
return image(_that.url,_that.alt);case Block_Table():
return table(_that.rows,_that.hasHeaderRow);case Block_Attachment():
return attachment(_that.name,_that.url);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( List<Inline> inlines)?  text,TResult? Function( String url,  String alt)?  image,TResult? Function( List<List<List<Inline>>> rows,  bool hasHeaderRow)?  table,TResult? Function( String name,  String url)?  attachment,}) {final _that = this;
switch (_that) {
case Block_Text() when text != null:
return text(_that.inlines);case Block_Image() when image != null:
return image(_that.url,_that.alt);case Block_Table() when table != null:
return table(_that.rows,_that.hasHeaderRow);case Block_Attachment() when attachment != null:
return attachment(_that.name,_that.url);case _:
  return null;

}
}

}

/// @nodoc


class Block_Text extends Block {
  const Block_Text({required  List<Inline> inlines}): _inlines = inlines,super._();
  

 final  List<Inline> _inlines;
 List<Inline> get inlines {
  if (_inlines is EqualUnmodifiableListView) return _inlines;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_inlines);
}


/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Block_TextCopyWith<Block_Text> get copyWith => _$Block_TextCopyWithImpl<Block_Text>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Block_Text&&const DeepCollectionEquality().equals(other._inlines, _inlines));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_inlines));

@override
String toString() {
  return 'Block.text(inlines: $inlines)';
}


}

/// @nodoc
abstract mixin class $Block_TextCopyWith<$Res> implements $BlockCopyWith<$Res> {
  factory $Block_TextCopyWith(Block_Text value, $Res Function(Block_Text) _then) = _$Block_TextCopyWithImpl;
@useResult
$Res call({
 List<Inline> inlines
});




}
/// @nodoc
class _$Block_TextCopyWithImpl<$Res>
    implements $Block_TextCopyWith<$Res> {
  _$Block_TextCopyWithImpl(this._self, this._then);

  final Block_Text _self;
  final $Res Function(Block_Text) _then;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? inlines = null,}) {
  return _then(Block_Text(
inlines: null == inlines ? _self._inlines : inlines // ignore: cast_nullable_to_non_nullable
as List<Inline>,
  ));
}


}

/// @nodoc


class Block_Image extends Block {
  const Block_Image({required this.url, required this.alt}): super._();
  

 final  String url;
 final  String alt;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Block_ImageCopyWith<Block_Image> get copyWith => _$Block_ImageCopyWithImpl<Block_Image>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Block_Image&&(identical(other.url, url) || other.url == url)&&(identical(other.alt, alt) || other.alt == alt));
}


@override
int get hashCode => Object.hash(runtimeType,url,alt);

@override
String toString() {
  return 'Block.image(url: $url, alt: $alt)';
}


}

/// @nodoc
abstract mixin class $Block_ImageCopyWith<$Res> implements $BlockCopyWith<$Res> {
  factory $Block_ImageCopyWith(Block_Image value, $Res Function(Block_Image) _then) = _$Block_ImageCopyWithImpl;
@useResult
$Res call({
 String url, String alt
});




}
/// @nodoc
class _$Block_ImageCopyWithImpl<$Res>
    implements $Block_ImageCopyWith<$Res> {
  _$Block_ImageCopyWithImpl(this._self, this._then);

  final Block_Image _self;
  final $Res Function(Block_Image) _then;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? url = null,Object? alt = null,}) {
  return _then(Block_Image(
url: null == url ? _self.url : url // ignore: cast_nullable_to_non_nullable
as String,alt: null == alt ? _self.alt : alt // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class Block_Table extends Block {
  const Block_Table({required  List<List<List<Inline>>> rows, required this.hasHeaderRow}): _rows = rows,super._();
  

 final  List<List<List<Inline>>> _rows;
 List<List<List<Inline>>> get rows {
  if (_rows is EqualUnmodifiableListView) return _rows;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_rows);
}

 final  bool hasHeaderRow;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Block_TableCopyWith<Block_Table> get copyWith => _$Block_TableCopyWithImpl<Block_Table>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Block_Table&&const DeepCollectionEquality().equals(other._rows, _rows)&&(identical(other.hasHeaderRow, hasHeaderRow) || other.hasHeaderRow == hasHeaderRow));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_rows),hasHeaderRow);

@override
String toString() {
  return 'Block.table(rows: $rows, hasHeaderRow: $hasHeaderRow)';
}


}

/// @nodoc
abstract mixin class $Block_TableCopyWith<$Res> implements $BlockCopyWith<$Res> {
  factory $Block_TableCopyWith(Block_Table value, $Res Function(Block_Table) _then) = _$Block_TableCopyWithImpl;
@useResult
$Res call({
 List<List<List<Inline>>> rows, bool hasHeaderRow
});




}
/// @nodoc
class _$Block_TableCopyWithImpl<$Res>
    implements $Block_TableCopyWith<$Res> {
  _$Block_TableCopyWithImpl(this._self, this._then);

  final Block_Table _self;
  final $Res Function(Block_Table) _then;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? rows = null,Object? hasHeaderRow = null,}) {
  return _then(Block_Table(
rows: null == rows ? _self._rows : rows // ignore: cast_nullable_to_non_nullable
as List<List<List<Inline>>>,hasHeaderRow: null == hasHeaderRow ? _self.hasHeaderRow : hasHeaderRow // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class Block_Attachment extends Block {
  const Block_Attachment({required this.name, required this.url}): super._();
  

 final  String name;
 final  String url;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Block_AttachmentCopyWith<Block_Attachment> get copyWith => _$Block_AttachmentCopyWithImpl<Block_Attachment>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Block_Attachment&&(identical(other.name, name) || other.name == name)&&(identical(other.url, url) || other.url == url));
}


@override
int get hashCode => Object.hash(runtimeType,name,url);

@override
String toString() {
  return 'Block.attachment(name: $name, url: $url)';
}


}

/// @nodoc
abstract mixin class $Block_AttachmentCopyWith<$Res> implements $BlockCopyWith<$Res> {
  factory $Block_AttachmentCopyWith(Block_Attachment value, $Res Function(Block_Attachment) _then) = _$Block_AttachmentCopyWithImpl;
@useResult
$Res call({
 String name, String url
});




}
/// @nodoc
class _$Block_AttachmentCopyWithImpl<$Res>
    implements $Block_AttachmentCopyWith<$Res> {
  _$Block_AttachmentCopyWithImpl(this._self, this._then);

  final Block_Attachment _self;
  final $Res Function(Block_Attachment) _then;

/// Create a copy of Block
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,Object? url = null,}) {
  return _then(Block_Attachment(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,url: null == url ? _self.url : url // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
