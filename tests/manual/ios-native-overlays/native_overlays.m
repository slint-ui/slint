// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore Autocapitalization Autocorrection Autoresizing instancetype kindof nonatomic subview subviews

#import <UIKit/UIKit.h>

typedef struct NativeRect {
    float x;
    float y;
    float width;
    float height;
} NativeRect;

extern void native_text_state_changed(uintptr_t editorIndex, const char *text,
                                      uintptr_t selectionAnchor, uintptr_t selectionFocus);
extern void native_selection_changed(uintptr_t editorIndex, uintptr_t selectionAnchor,
                                     uintptr_t selectionFocus);
extern bool native_caret_rect(uintptr_t editorIndex, uintptr_t utf16Offset, NativeRect *result);
extern uintptr_t native_closest_text_position(uintptr_t editorIndex, float windowX, float windowY);
extern uint32_t native_selection_color(uintptr_t editorIndex);
extern void native_editor_active(uintptr_t editorIndex, bool active);
extern void native_menu_button_pressed(bool pressed);
extern void native_context_action(int action);

@interface SlintTextPosition : UITextPosition
@property (nonatomic) NSInteger offset;
+ (instancetype)positionWithOffset:(NSInteger)offset;
@end

@implementation SlintTextPosition
+ (instancetype)positionWithOffset:(NSInteger)offset
{
    SlintTextPosition *position = [SlintTextPosition new];
    position.offset = offset;
    return position;
}
@end

@interface SlintTextRange : UITextRange
@property (nonatomic, strong) SlintTextPosition *rangeStart;
@property (nonatomic, strong) SlintTextPosition *rangeEnd;
+ (instancetype)rangeWithNSRange:(NSRange)range;
@end

@implementation SlintTextRange
+ (instancetype)rangeWithNSRange:(NSRange)range
{
    SlintTextRange *textRange = [SlintTextRange new];
    textRange.rangeStart = [SlintTextPosition positionWithOffset:range.location];
    textRange.rangeEnd = [SlintTextPosition positionWithOffset:NSMaxRange(range)];
    return textRange;
}
- (UITextPosition *)start { return self.rangeStart; }
- (UITextPosition *)end { return self.rangeEnd; }
- (BOOL)isEmpty { return self.rangeStart.offset == self.rangeEnd.offset; }
@end

@interface SlintSelectionRect : UITextSelectionRect
@property (nonatomic) CGRect selectionRect;
@property (nonatomic) BOOL containsStartValue;
@property (nonatomic) BOOL containsEndValue;
@end

@implementation SlintSelectionRect
- (CGRect)rect { return self.selectionRect; }
- (NSWritingDirection)writingDirection { return NSWritingDirectionLeftToRight; }
- (BOOL)containsStart { return self.containsStartValue; }
- (BOOL)containsEnd { return self.containsEndValue; }
- (BOOL)isVertical { return NO; }
@end

@interface SlintTextInputView : UIView <UITextInput, UITextInputTraits>
@property (nonatomic, weak) UIView *host;
@property (nonatomic, strong) NSMutableString *storage;
@property (nonatomic) NSRange selection;
@property (nonatomic) NSRange markedRangeValue;
@property (nonatomic, weak) id<UITextInputDelegate> inputDelegate;
@property (nonatomic, strong) UITextInputStringTokenizer *tokenizerValue;
@property (nonatomic, copy) NSDictionary *markedTextStyle;
@property (nonatomic) UITextStorageDirection selectionAffinity;
@property (nonatomic) UITextAutocorrectionType autocorrectionType;
@property (nonatomic) UITextSpellCheckingType spellCheckingType;
@property (nonatomic) UIReturnKeyType returnKeyType;
@property (nonatomic) UIKeyboardType keyboardType;
@property (nonatomic) UIKeyboardAppearance keyboardAppearance;
@property (nonatomic) UITextAutocapitalizationType autocapitalizationType;
@property (nonatomic, getter=isSecureTextEntry) BOOL secureTextEntry;
@property (nonatomic) NSUInteger editorIndex;
@property (nonatomic) BOOL multiline;
@property (nonatomic, strong) UITextInteraction *textInteraction;
- (instancetype)initWithFrame:(CGRect)frame
                         host:(UIView *)host
                         text:(NSString *)text
                  editorIndex:(NSUInteger)editorIndex
                    multiline:(BOOL)multiline
           accessibilityLabel:(NSString *)accessibilityLabel;
@end

@implementation SlintTextInputView
- (instancetype)initWithFrame:(CGRect)frame
                         host:(UIView *)host
                         text:(NSString *)text
                  editorIndex:(NSUInteger)editorIndex
                    multiline:(BOOL)multiline
           accessibilityLabel:(NSString *)accessibilityLabel
{
    self = [super initWithFrame:frame];
    if (!self)
        return nil;
    self.host = host;
    self.editorIndex = editorIndex;
    self.multiline = multiline;
    self.storage = [text mutableCopy];
    self.selection = NSMakeRange(self.storage.length, 0);
    self.markedRangeValue = NSMakeRange(NSNotFound, 0);
    self.backgroundColor = UIColor.clearColor;
    self.opaque = NO;
    self.isAccessibilityElement = YES;
    self.accessibilityIdentifier = accessibilityLabel;
    self.accessibilityLabel = accessibilityLabel;
    self.accessibilityValue = self.storage;
    self.autocorrectionType = UITextAutocorrectionTypeDefault;
    self.spellCheckingType = UITextSpellCheckingTypeDefault;
    self.autocapitalizationType = UITextAutocapitalizationTypeSentences;
    self.returnKeyType = multiline ? UIReturnKeyDefault : UIReturnKeyDone;
    self.keyboardType = UIKeyboardTypeDefault;
    self.keyboardAppearance = UIKeyboardAppearanceDefault;
    self.tokenizerValue = [[UITextInputStringTokenizer alloc] initWithTextInput:self];
    UITextInteraction *interaction =
            [UITextInteraction textInteractionForMode:UITextInteractionModeEditable];
    interaction.textInput = self;
    self.textInteraction = interaction;
    [self addInteraction:interaction];
    return self;
}

- (void)hideNativeSelectionHighlightsInView:(UIView *)view
{
    if (@available(iOS 17.0, *)) {
        if ([view conformsToProtocol:@protocol(UITextSelectionHighlightView)])
            view.hidden = YES;
    }
    for (UIView *subview in view.subviews)
        [self hideNativeSelectionHighlightsInView:subview];
}

- (void)didAddSubview:(UIView *)subview
{
    [super didAddSubview:subview];
    [self hideNativeSelectionHighlightsInView:subview];
}

- (void)layoutSubviews
{
    [super layoutSubviews];
    [self hideNativeSelectionHighlightsInView:self];
}

- (BOOL)canBecomeFirstResponder { return YES; }
- (BOOL)becomeFirstResponder
{
    self.textInteraction.textInput = self;
    if (![self.interactions containsObject:self.textInteraction])
        [self addInteraction:self.textInteraction];
    uint32_t argb = native_selection_color(self.editorIndex);
    self.tintColor = [UIColor colorWithRed:((argb >> 16) & 0xff) / 255.0
                                    green:((argb >> 8) & 0xff) / 255.0
                                     blue:(argb & 0xff) / 255.0
                                    alpha:1];
    native_editor_active(self.editorIndex, true);
    BOOL result = [super becomeFirstResponder];
    if (result)
        [self syncSelection];
    else
        native_editor_active(self.editorIndex, false);
    return result;
}
- (BOOL)resignFirstResponder
{
    BOOL result = [super resignFirstResponder];
    if (result) {
        if ([self.interactions containsObject:self.textInteraction])
            [self removeInteraction:self.textInteraction];
        self.textInteraction.textInput = nil;
        if (self.selection.length > 0) {
            _selection = NSMakeRange(NSMaxRange(self.selection), 0);
            [self syncSelection];
        }
        native_editor_active(self.editorIndex, false);
    }
    return result;
}
// Consuming these callbacks prevents the Winit host view from taking the responder during UIKit
// text gestures.
- (void)touchesBegan:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {}
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {}
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {}
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {}

- (NSRange)clampedRange:(NSRange)range
{
    NSUInteger location = MIN(range.location, self.storage.length);
    return NSMakeRange(location, MIN(range.length, self.storage.length - location));
}
- (NSRange)rangeFromTextRange:(UITextRange *)textRange
{
    SlintTextRange *range = (SlintTextRange *)textRange;
    NSInteger start = MAX(0, range.rangeStart.offset);
    NSInteger end = MAX(start, range.rangeEnd.offset);
    return [self clampedRange:NSMakeRange(start, end - start)];
}
- (void)syncSlint
{
    self.accessibilityValue = self.storage;
    native_text_state_changed(self.editorIndex, self.storage.UTF8String, self.selection.location,
                              NSMaxRange(self.selection));
}
- (void)syncSelection
{
    native_selection_changed(self.editorIndex, self.selection.location,
                             NSMaxRange(self.selection));
}
- (void)setSelectionAndNotify:(NSRange)selection
{
    [self.inputDelegate selectionWillChange:self];
    _selection = [self clampedRange:selection];
    [self syncSelection];
    [self.inputDelegate selectionDidChange:self];
}
- (void)replaceCharactersInRange:(NSRange)range withString:(NSString *)text
{
    range = [self clampedRange:range];
    [self.inputDelegate textWillChange:self];
    [self.inputDelegate selectionWillChange:self];
    [self.storage replaceCharactersInRange:range withString:text];
    _selection = NSMakeRange(range.location + text.length, 0);
    _markedRangeValue = NSMakeRange(NSNotFound, 0);
    [self syncSlint];
    [self.inputDelegate selectionDidChange:self];
    [self.inputDelegate textDidChange:self];
}

- (UITextRange *)selectedTextRange { return [SlintTextRange rangeWithNSRange:self.selection]; }
- (void)setSelectedTextRange:(UITextRange *)range
{
    _selection = [self rangeFromTextRange:range];
    [self syncSelection];
}
- (UITextRange *)markedTextRange
{
    return self.markedRangeValue.location == NSNotFound
            ? nil
            : [SlintTextRange rangeWithNSRange:self.markedRangeValue];
}
- (UITextPosition *)beginningOfDocument { return [SlintTextPosition positionWithOffset:0]; }
- (UITextPosition *)endOfDocument
{
    return [SlintTextPosition positionWithOffset:self.storage.length];
}
- (id<UITextInputTokenizer>)tokenizer { return self.tokenizerValue; }
- (UIView *)textInputView { return self; }
- (NSString *)textInRange:(UITextRange *)range
{
    return [self.storage substringWithRange:[self rangeFromTextRange:range]];
}
- (void)replaceRange:(UITextRange *)range withText:(NSString *)text
{
    [self replaceCharactersInRange:[self rangeFromTextRange:range] withString:text];
}
- (void)setMarkedText:(NSString *)markedText selectedRange:(NSRange)selectedRange
{
    NSRange replacement = self.markedRangeValue.location == NSNotFound ? self.selection
                                                                        : self.markedRangeValue;
    replacement = [self clampedRange:replacement];
    [self.inputDelegate textWillChange:self];
    [self.inputDelegate selectionWillChange:self];
    [self.storage replaceCharactersInRange:replacement withString:markedText ?: @""];
    self.markedRangeValue = NSMakeRange(replacement.location, markedText.length);
    self.selection = [self clampedRange:NSMakeRange(replacement.location + selectedRange.location,
                                                    selectedRange.length)];
    [self syncSlint];
    [self.inputDelegate selectionDidChange:self];
    [self.inputDelegate textDidChange:self];
}
- (void)unmarkText
{
    self.markedRangeValue = NSMakeRange(NSNotFound, 0);
}
- (UITextRange *)textRangeFromPosition:(UITextPosition *)fromPosition
                            toPosition:(UITextPosition *)toPosition
{
    NSInteger from = ((SlintTextPosition *)fromPosition).offset;
    NSInteger to = ((SlintTextPosition *)toPosition).offset;
    NSInteger start = MIN(from, to);
    return [SlintTextRange rangeWithNSRange:NSMakeRange(start, labs(to - from))];
}
- (UITextPosition *)positionFromPosition:(UITextPosition *)position offset:(NSInteger)offset
{
    NSInteger result = ((SlintTextPosition *)position).offset + offset;
    return result < 0 || result > (NSInteger)self.storage.length
            ? nil
            : [SlintTextPosition positionWithOffset:result];
}
- (UITextPosition *)positionFromPosition:(UITextPosition *)position
                             inDirection:(UITextLayoutDirection)direction
                                  offset:(NSInteger)offset
{
    BOOL backwards = direction == UITextLayoutDirectionLeft || direction == UITextLayoutDirectionUp;
    return [self positionFromPosition:position offset:backwards ? -offset : offset];
}
- (NSComparisonResult)comparePosition:(UITextPosition *)position
                           toPosition:(UITextPosition *)other
{
    NSInteger lhs = ((SlintTextPosition *)position).offset;
    NSInteger rhs = ((SlintTextPosition *)other).offset;
    return lhs < rhs ? NSOrderedAscending : lhs > rhs ? NSOrderedDescending : NSOrderedSame;
}
- (NSInteger)offsetFromPosition:(UITextPosition *)from toPosition:(UITextPosition *)toPosition
{
    return ((SlintTextPosition *)toPosition).offset - ((SlintTextPosition *)from).offset;
}
- (UITextPosition *)positionWithinRange:(UITextRange *)range
                    farthestInDirection:(UITextLayoutDirection)direction
{
    SlintTextRange *textRange = (SlintTextRange *)range;
    return direction == UITextLayoutDirectionLeft || direction == UITextLayoutDirectionUp
            ? textRange.start
            : textRange.end;
}
- (UITextRange *)characterRangeByExtendingPosition:(UITextPosition *)position
                                       inDirection:(UITextLayoutDirection)direction
{
    NSInteger offset = ((SlintTextPosition *)position).offset;
    if (direction == UITextLayoutDirectionLeft || direction == UITextLayoutDirectionUp)
        return [SlintTextRange rangeWithNSRange:NSMakeRange(MAX(0, offset - 1), offset > 0)];
    return [SlintTextRange rangeWithNSRange:
            NSMakeRange(MIN(offset, (NSInteger)self.storage.length),
                        offset < (NSInteger)self.storage.length)];
}
- (NSWritingDirection)baseWritingDirectionForPosition:(UITextPosition *)position
                                        inDirection:(UITextStorageDirection)direction
{
    return NSWritingDirectionLeftToRight;
}
- (void)setBaseWritingDirection:(NSWritingDirection)direction forRange:(UITextRange *)range {}

- (CGRect)localCaretRectForOffset:(NSUInteger)offset
{
    NativeRect rect = { 0 };
    if (!native_caret_rect(self.editorIndex, offset, &rect))
        return CGRectZero;
    return [self convertRect:CGRectMake(rect.x, rect.y, rect.width, rect.height)
                    fromView:self.host];
}
- (CGRect)firstRectForRange:(UITextRange *)range
{
    NSRange selected = [self rangeFromTextRange:range];
    CGRect start = [self localCaretRectForOffset:selected.location];
    CGRect end = [self localCaretRectForOffset:NSMaxRange(selected)];
    if (selected.length == 0)
        return start;
    CGFloat x = MIN(CGRectGetMinX(start), CGRectGetMinX(end));
    CGFloat maxX = MAX(CGRectGetMinX(start), CGRectGetMinX(end));
    return CGRectMake(x, MIN(CGRectGetMinY(start), CGRectGetMinY(end)), MAX(1, maxX - x),
                      MAX(CGRectGetHeight(start), CGRectGetHeight(end)));
}
- (CGRect)caretRectForPosition:(UITextPosition *)position
{
    return [self localCaretRectForOffset:((SlintTextPosition *)position).offset];
}
- (NSArray<UITextSelectionRect *> *)selectionRectsForRange:(UITextRange *)range
{
    NSRange selected = [self rangeFromTextRange:range];
    NSUInteger selectedEnd = NSMaxRange(selected);
    NSUInteger committedEnd = NSMaxRange(self.selection);
    BOOL sharesCommittedEndpoint = selected.location == self.selection.location
            || selected.location == committedEnd || selectedEnd == self.selection.location
            || selectedEnd == committedEnd;
    // UIKit reports the live handle range here and commits selectedTextRange when the drag ends.
    if (!NSEqualRanges(selected, self.selection) && selected.length > 0
        && self.selection.length > 0 && sharesCommittedEndpoint) {
        native_selection_changed(self.editorIndex, selected.location, selectedEnd);
    }
    CGRect start = [self localCaretRectForOffset:selected.location];
    CGRect end = [self localCaretRectForOffset:NSMaxRange(selected)];
    if (selected.length == 0) {
        SlintSelectionRect *rect = [SlintSelectionRect new];
        rect.selectionRect = start;
        rect.containsStartValue = YES;
        rect.containsEndValue = YES;
        return @[ rect ];
    }

    SlintSelectionRect *startRect = [SlintSelectionRect new];
    startRect.selectionRect = CGRectMake(CGRectGetMinX(start), CGRectGetMinY(start),
                                         MAX(1, CGRectGetWidth(start)), CGRectGetHeight(start));
    startRect.containsStartValue = YES;
    SlintSelectionRect *endRect = [SlintSelectionRect new];
    endRect.selectionRect = CGRectMake(CGRectGetMinX(end), CGRectGetMinY(end),
                                       MAX(1, CGRectGetWidth(end)), CGRectGetHeight(end));
    endRect.containsEndValue = YES;
    return @[ startRect, endRect ];
}
- (UITextPosition *)closestPositionToPoint:(CGPoint)point
{
    CGPoint hostPoint = [self convertPoint:point toView:self.host];
    return [SlintTextPosition
            positionWithOffset:native_closest_text_position(self.editorIndex, hostPoint.x,
                                                            hostPoint.y)];
}
- (UITextPosition *)closestPositionToPoint:(CGPoint)point withinRange:(UITextRange *)range
{
    SlintTextPosition *position = (SlintTextPosition *)[self closestPositionToPoint:point];
    NSRange bounds = [self rangeFromTextRange:range];
    position.offset = MIN(MAX(position.offset, (NSInteger)bounds.location),
                          (NSInteger)NSMaxRange(bounds));
    return position;
}
- (UITextRange *)characterRangeAtPoint:(CGPoint)point
{
    NSInteger offset = ((SlintTextPosition *)[self closestPositionToPoint:point]).offset;
    if (offset >= (NSInteger)self.storage.length)
        return [SlintTextRange rangeWithNSRange:NSMakeRange(self.storage.length, 0)];
    return [SlintTextRange
            rangeWithNSRange:[self.storage rangeOfComposedCharacterSequenceAtIndex:offset]];
}

- (BOOL)hasText { return self.storage.length > 0; }
- (void)insertText:(NSString *)text
{
    if (!self.multiline && [text isEqualToString:@"\n"]) {
        [self resignFirstResponder];
        return;
    }
    NSRange replacement = self.markedRangeValue.location == NSNotFound ? self.selection
                                                                        : self.markedRangeValue;
    [self replaceCharactersInRange:replacement withString:text];
}
- (void)deleteBackward
{
    NSRange replacement = self.selection;
    if (replacement.length == 0 && replacement.location > 0)
        replacement = [self.storage rangeOfComposedCharacterSequenceAtIndex:replacement.location - 1];
    if (replacement.length > 0)
        [self replaceCharactersInRange:replacement withString:@""];
}
- (void)copy:(id)sender
{
    if (self.selection.length > 0)
        UIPasteboard.generalPasteboard.string = [self.storage substringWithRange:self.selection];
}
- (void)cut:(id)sender
{
    [self copy:sender];
    if (self.selection.length > 0)
        [self replaceCharactersInRange:self.selection withString:@""];
}
- (void)paste:(id)sender
{
    if (UIPasteboard.generalPasteboard.string)
        [self replaceCharactersInRange:self.selection
                            withString:UIPasteboard.generalPasteboard.string];
}
- (void)selectAll:(id)sender
{
    [self setSelectionAndNotify:NSMakeRange(0, self.storage.length)];
}
- (BOOL)canPerformAction:(SEL)action withSender:(id)sender
{
    if (action == @selector(copy:) || action == @selector(cut:))
        return self.selection.length > 0;
    if (action == @selector(paste:))
        return UIPasteboard.generalPasteboard.hasStrings;
    if (action == @selector(selectAll:))
        return self.storage.length > 0 && self.selection.length != self.storage.length;
    return [super canPerformAction:action withSender:sender];
}
@end

@interface NativeOverlayManager : NSObject
@property (nonatomic, weak) UIView *host;
@property (nonatomic, strong) UIButton *menuTarget;
@property (nonatomic, strong) SlintTextInputView *editor;
@property (nonatomic, strong) SlintTextInputView *secondEditor;
@property (nonatomic, strong) UITextView *nativeEditor;
@end

@implementation NativeOverlayManager
- (void)performContextAction:(int)action result:(NSString *)result
{
    self.menuTarget.accessibilityValue = result;
    native_context_action(action);
}

- (instancetype)initWithHost:(UIView *)host
                 editorFrame:(CGRect)editorFrame
           secondEditorFrame:(CGRect)secondEditorFrame
           nativeEditorFrame:(CGRect)nativeEditorFrame
                   menuFrame:(CGRect)menuFrame
                 initialText:(NSString *)initialText
           secondInitialText:(NSString *)secondInitialText
{
    self = [super init];
    if (!self)
        return nil;
    self.host = host;
    self.editor = [[SlintTextInputView alloc] initWithFrame:editorFrame
                                                       host:host
                                                       text:initialText
                                                editorIndex:0
                                                  multiline:NO
                                         accessibilityLabel:@"Native text editor"];
    [host addSubview:self.editor];
    self.secondEditor =
            [[SlintTextInputView alloc] initWithFrame:secondEditorFrame
                                                 host:host
                                                 text:secondInitialText
                                          editorIndex:1
                                            multiline:YES
                                   accessibilityLabel:@"Native multiline text editor"];
    [host addSubview:self.secondEditor];

    self.nativeEditor = [[UITextView alloc] initWithFrame:nativeEditorFrame];
    self.nativeEditor.text = secondInitialText;
    self.nativeEditor.font = [UIFont systemFontOfSize:17];
    self.nativeEditor.textColor = [UIColor colorWithRed:0x17 / 255.0
                                                  green:0x23 / 255.0
                                                   blue:0x3b / 255.0
                                                  alpha:1];
    self.nativeEditor.backgroundColor = UIColor.clearColor;
    self.nativeEditor.textContainerInset = UIEdgeInsetsMake(12, 12, 12, 12);
    self.nativeEditor.textContainer.lineFragmentPadding = 4;
    self.nativeEditor.accessibilityIdentifier = @"Pure UIKit multiline editor";
    self.nativeEditor.accessibilityLabel = @"Pure UIKit multiline editor";
    [host addSubview:self.nativeEditor];

    self.menuTarget = [UIButton buttonWithType:UIButtonTypeCustom];
    self.menuTarget.frame = menuFrame;
    self.menuTarget.backgroundColor = UIColor.clearColor;
    self.menuTarget.accessibilityIdentifier = @"Context menu target";
    self.menuTarget.accessibilityLabel = @"Show native menu";
    self.menuTarget.menu = [self nativeMenu];
    self.menuTarget.showsMenuAsPrimaryAction = YES;
    [self.menuTarget addTarget:self
                        action:@selector(menuButtonDown)
              forControlEvents:UIControlEventTouchDown];
    [self.menuTarget addTarget:self
                        action:@selector(menuButtonUp)
              forControlEvents:UIControlEventTouchUpInside | UIControlEventTouchUpOutside |
              UIControlEventTouchCancel];
    [host addSubview:self.menuTarget];

    UITapGestureRecognizer *outsideTap =
            [[UITapGestureRecognizer alloc] initWithTarget:self action:@selector(hostTapped:)];
    outsideTap.cancelsTouchesInView = NO;
    [host addGestureRecognizer:outsideTap];
    return self;
}

- (void)menuButtonDown
{
    native_menu_button_pressed(true);
}

- (void)menuButtonUp
{
    native_menu_button_pressed(false);
}

- (void)hostTapped:(UITapGestureRecognizer *)recognizer
{
    if (recognizer.state != UIGestureRecognizerStateEnded)
        return;
    CGPoint point = [recognizer locationInView:self.host];
    if (CGRectContainsPoint(self.editor.frame, point)) {
        [self.secondEditor resignFirstResponder];
        [self.nativeEditor resignFirstResponder];
        [self.editor becomeFirstResponder];
    } else if (CGRectContainsPoint(self.secondEditor.frame, point)) {
        [self.editor resignFirstResponder];
        [self.nativeEditor resignFirstResponder];
        [self.secondEditor becomeFirstResponder];
    } else if (CGRectContainsPoint(self.nativeEditor.frame, point)) {
        [self.editor resignFirstResponder];
        [self.secondEditor resignFirstResponder];
    } else {
        [self.editor resignFirstResponder];
        [self.secondEditor resignFirstResponder];
        [self.nativeEditor resignFirstResponder];
    }
}

- (UIMenu *)nativeMenu
{
    __weak NativeOverlayManager *weakSelf = self;
    UIAction *rename = [UIAction actionWithTitle:@"Rename"
                                           image:[UIImage systemImageNamed:@"pencil"]
                                      identifier:nil
                                         handler:^(__kindof UIAction *__unused action) {
                                             [weakSelf performContextAction:0
                                                                     result:@"Rename selected"];
                                         }];
    UIAction *duplicate =
            [UIAction actionWithTitle:@"Duplicate"
                                image:[UIImage systemImageNamed:@"plus.square.on.square"]
                           identifier:nil
                              handler:^(__kindof UIAction *__unused action) {
                                  [weakSelf performContextAction:1 result:@"Duplicate selected"];
                              }];
    UIAction *deleteAction =
            [UIAction actionWithTitle:@"Delete"
                                image:[UIImage systemImageNamed:@"trash"]
                           identifier:nil
                              handler:^(__kindof UIAction *__unused action) {
                                  [weakSelf performContextAction:2 result:@"Delete selected"];
                              }];
    deleteAction.attributes = UIMenuElementAttributesDestructive;
    return [UIMenu menuWithTitle:@"Example document" children:@[ rename, duplicate, deleteAction ]];
}
@end

static NativeOverlayManager *manager;

void install_native_overlays(void *hostPointer, float editorX, float editorY, float editorWidth,
                             float editorHeight, float secondEditorX, float secondEditorY,
                             float secondEditorWidth, float secondEditorHeight, float nativeEditorX,
                             float nativeEditorY, float nativeEditorWidth, float nativeEditorHeight,
                             float menuX, float menuY, float menuWidth, float menuHeight,
                             const char *initialText, const char *secondInitialText)
{
    UIView *host = (__bridge UIView *)hostPointer;
    NSString *text = [NSString stringWithUTF8String:initialText];
    NSString *secondText = [NSString stringWithUTF8String:secondInitialText];
    manager = [[NativeOverlayManager alloc]
            initWithHost:host
             editorFrame:CGRectMake(editorX, editorY, editorWidth, editorHeight)
       secondEditorFrame:CGRectMake(secondEditorX, secondEditorY, secondEditorWidth,
                                    secondEditorHeight)
       nativeEditorFrame:CGRectMake(nativeEditorX, nativeEditorY, nativeEditorWidth,
                                    nativeEditorHeight)
               menuFrame:CGRectMake(menuX, menuY, menuWidth, menuHeight)
             initialText:text
       secondInitialText:secondText];
}
