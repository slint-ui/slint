// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// A Flutter widget that renders a Slint user interface.
///
/// ```dart
/// SlintView(
///   load: () => loadFile('ui/todo.slint')
///     ..setCallback('todo-added', (args) { … }),
/// )
/// ```
///
/// Slint draws into a pixel buffer through [SlintSurface] rather than opening
/// its own window, so the result composes with the rest of the widget tree and
/// works on every platform Flutter runs on.
library;

import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:slint/slint.dart';

export 'package:slint/slint.dart';

/// Displays a Slint component and forwards pointer and keyboard input to it.
///
/// Only one [SlintView] can be alive at a time: Slint's software-rendering
/// platform owns a single surface for the isolate.
class SlintView extends StatefulWidget {
  const SlintView({
    required SlintComponent Function() load,
    this.autofocus = true,
    super.key,
  }) : _load = load;

  /// Loads the component to display. It runs once, after the Slint surface
  /// exists, which is why it is a callback rather than a plain instance:
  /// `loadFile` must not run before the surface is in place.
  /// It may return a generated wrapper or a plain [ComponentInstance].
  final SlintComponent Function() _load;

  /// The runtime-instance loader used by this view.
  ///
  /// For compatibility, a loader that already returns [ComponentInstance] is
  /// returned unchanged. Generated wrapper loaders are adapted to return their
  /// underlying instance.
  ComponentInstance Function() get load {
    final loader = _load;
    if (loader is ComponentInstance Function()) return loader;
    return () => loader().instance;
  }

  /// Whether the view takes keyboard focus when it appears.
  final bool autofocus;

  @override
  State<SlintView> createState() => _SlintViewState();
}

class _SlintViewState extends State<SlintView>
    with SingleTickerProviderStateMixin {
  late final SlintSurface _surface;
  late final ComponentInstance _instance;
  late final Ticker _ticker;
  final _focusNode = FocusNode();

  ui.Image? _frame;
  bool _decoding = false;
  Size _size = Size.zero;
  double _scaleFactor = 1;

  @override
  void initState() {
    super.initState();
    _surface = SlintSurface();
    _instance = widget.load()..show();
    _ticker = createTicker(_onFrame)..start();
  }

  @override
  void dispose() {
    _ticker.dispose();
    _focusNode.dispose();
    _instance.dispose();
    _frame?.dispose();
    _surface.dispose();
    super.dispose();
  }

  void _onFrame(Duration _) {
    _surface.tick();
    if (_decoding) return;

    final pixels = _surface.render();
    if (pixels == null) return;

    // `decodeImageFromPixels` reads the buffer asynchronously, and the next
    // render would overwrite it, so hand over a copy.
    _decoding = true;
    ui.decodeImageFromPixels(
      Uint8List.fromList(pixels),
      _surface.width,
      _surface.height,
      ui.PixelFormat.rgba8888,
      (image) {
        _decoding = false;
        if (!mounted) {
          image.dispose();
          return;
        }
        setState(() {
          _frame?.dispose();
          _frame = image;
        });
      },
    );
  }

  void _resize(BoxConstraints constraints, double devicePixelRatio) {
    final logical = Size(
      constraints.maxWidth.isFinite ? constraints.maxWidth : 800,
      constraints.maxHeight.isFinite ? constraints.maxHeight : 600,
    );
    if (logical == _size && devicePixelRatio == _scaleFactor) return;
    _size = logical;
    _scaleFactor = devicePixelRatio;
    _surface.resize(
      (logical.width * devicePixelRatio).round(),
      (logical.height * devicePixelRatio).round(),
      scaleFactor: devicePixelRatio,
    );
  }

  void _pointer(PointerEventKind kind, Offset position,
      {int buttons = kPrimaryButton, Offset scroll = Offset.zero}) {
    _surface.dispatchPointer(
      kind,
      x: position.dx,
      y: position.dy,
      button: switch (buttons) {
        kSecondaryButton => PointerButton.right,
        kTertiaryButton => PointerButton.middle,
        _ => PointerButton.left,
      },
      deltaX: scroll.dx,
      deltaY: scroll.dy,
    );
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    final text = _slintText(event);
    if (text == null) return KeyEventResult.ignored;
    _surface.dispatchKey(
      switch (event) {
        KeyDownEvent() => KeyEventKind.pressed,
        KeyRepeatEvent() => KeyEventKind.repeated,
        _ => KeyEventKind.released,
      },
      text,
    );
    return KeyEventResult.handled;
  }

  /// Flutter's key event as the text Slint expects: the typed character, or
  /// the code Slint reserves for a key that produces none.
  static String? _slintText(KeyEvent event) {
    final special = _specialKeys[event.logicalKey];
    if (special != null) return special;
    final character = event.character;
    if (character != null && character.isNotEmpty) return character;
    return null;
  }

  static final Map<LogicalKeyboardKey, String> _specialKeys = {
    LogicalKeyboardKey.backspace: SlintKey.backspace,
    LogicalKeyboardKey.tab: SlintKey.tab,
    LogicalKeyboardKey.enter: SlintKey.enter,
    LogicalKeyboardKey.numpadEnter: SlintKey.enter,
    LogicalKeyboardKey.escape: SlintKey.escape,
    LogicalKeyboardKey.delete: SlintKey.delete,
    LogicalKeyboardKey.shiftLeft: SlintKey.shift,
    LogicalKeyboardKey.shiftRight: SlintKey.shift,
    LogicalKeyboardKey.controlLeft: SlintKey.control,
    LogicalKeyboardKey.controlRight: SlintKey.control,
    LogicalKeyboardKey.altLeft: SlintKey.alt,
    LogicalKeyboardKey.altRight: SlintKey.alt,
    LogicalKeyboardKey.metaLeft: SlintKey.meta,
    LogicalKeyboardKey.metaRight: SlintKey.meta,
    LogicalKeyboardKey.capsLock: SlintKey.capsLock,
    LogicalKeyboardKey.arrowUp: SlintKey.upArrow,
    LogicalKeyboardKey.arrowDown: SlintKey.downArrow,
    LogicalKeyboardKey.arrowLeft: SlintKey.leftArrow,
    LogicalKeyboardKey.arrowRight: SlintKey.rightArrow,
    LogicalKeyboardKey.insert: SlintKey.insert,
    LogicalKeyboardKey.home: SlintKey.home,
    LogicalKeyboardKey.end: SlintKey.end,
    LogicalKeyboardKey.pageUp: SlintKey.pageUp,
    LogicalKeyboardKey.pageDown: SlintKey.pageDown,
  };

  @override
  Widget build(BuildContext context) {
    final devicePixelRatio = MediaQuery.devicePixelRatioOf(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        _resize(constraints, devicePixelRatio);
        return Focus(
          focusNode: _focusNode,
          autofocus: widget.autofocus,
          onKeyEvent: _key,
          onFocusChange: (focused) =>
              _surface.dispatchFocus(focused: focused),
          child: Listener(
            // The Slint surface takes pointer events across its whole area:
            // neither the rendered frame nor the placeholder before the first
            // one hit-tests on its own.
            behavior: HitTestBehavior.opaque,
            onPointerDown: (e) {
              _focusNode.requestFocus();
              _pointer(PointerEventKind.pressed, e.localPosition,
                  buttons: e.buttons);
            },
            onPointerUp: (e) => _pointer(
                PointerEventKind.released, e.localPosition,
                buttons: e.buttons),
            onPointerHover: (e) =>
                _pointer(PointerEventKind.moved, e.localPosition),
            onPointerMove: (e) =>
                _pointer(PointerEventKind.moved, e.localPosition),
            onPointerCancel: (e) =>
                _pointer(PointerEventKind.exited, e.localPosition),
            onPointerSignal: (e) {
              if (e is PointerScrollEvent) {
                _pointer(PointerEventKind.scrolled, e.localPosition,
                    scroll: -e.scrollDelta);
              }
            },
            child: SizedBox.expand(
              child: _frame == null
                  ? const SizedBox.shrink()
                  : RawImage(
                      image: _frame,
                      width: _size.width,
                      height: _size.height,
                      fit: BoxFit.fill,
                      filterQuality: FilterQuality.none,
                    ),
            ),
          ),
        );
      },
    );
  }
}
