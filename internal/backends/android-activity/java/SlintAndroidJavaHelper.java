// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore drawables javahelper Spannable tbstart tbend

package dev.slint.android;

import android.animation.ValueAnimator;
import android.app.Activity;
import android.app.Application;
import android.app.NativeActivity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.Configuration;
import android.content.res.TypedArray;
import android.graphics.Canvas;
import android.graphics.Insets;
import android.graphics.Paint;
import android.graphics.PorterDuff;
import android.graphics.Rect;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.Drawable;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;
import android.provider.Settings;
import android.text.Editable;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.util.Log;
import android.view.ActionMode;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.InputDevice;
import android.view.Menu;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.WindowMetrics;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.CursorAnchorInfo;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.Magnifier;
import android.widget.PopupWindow;
import android.window.OnBackInvokedDispatcher;
import java.util.List;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.FutureTask;

class InputHandle extends ImageView {
    // The ids that callback_move_cursor_handle in javahelper.rs expects.
    static final int CURSOR = 0;
    static final int SELECTION_START = 1;
    static final int SELECTION_END = 2;

    private final SlintInputView mRootView;
    private final PopupWindow mPopupWindow;
    private final int mId;
    private final int mOffsetX;
    private final int mTouchSlop;
    private int mCursorX;
    private int mCursorY;
    private float mPressedX;
    private float mPressedY;
    private float mDownRawX;
    private float mDownRawY;
    private boolean mDragging;

    public InputHandle(SlintInputView rootView, int id) {
        super(rootView.getContext());
        mRootView = rootView;
        mId = id;
        Context ctx = rootView.getContext();
        mPopupWindow = new PopupWindow(ctx, null, android.R.attr.textSelectHandleWindowStyle);
        mPopupWindow.setSplitTouchEnabled(true);
        mPopupWindow.setClippingEnabled(false);
        int attr;
        int offsetQuarters;
        switch (id) {
            case SELECTION_START:
                attr = android.R.attr.textSelectHandleLeft;
                offsetQuarters = 3;
                break;
            case SELECTION_END:
                attr = android.R.attr.textSelectHandleRight;
                offsetQuarters = 1;
                break;
            default:
                attr = android.R.attr.textSelectHandle;
                offsetQuarters = 2;
                break;
        }
        TypedArray a = ctx.getTheme().obtainStyledAttributes(new int[] { attr });
        Drawable drawable = a.getDrawable(0);
        a.recycle();
        int width = drawable.getIntrinsicWidth();
        mOffsetX = offsetQuarters * width / 4;
        mTouchSlop = ViewConfiguration.get(ctx).getScaledTouchSlop();
        mPopupWindow.setWidth(width);
        mPopupWindow.setHeight(drawable.getIntrinsicHeight());
        setImageDrawable(drawable);
        mPopupWindow.setContentView(this);
    }

    @Override
    public boolean onTouchEvent(MotionEvent ev) {
        switch (ev.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                mPressedX = ev.getRawX() - mCursorX;
                mPressedY = ev.getRawY() - mCursorY;
                mDownRawX = ev.getRawX();
                mDownRawY = ev.getRawY();
                // Only the cursor handle has a tap action, so only it waits for the touch slop.
                mDragging = mId != CURSOR;
                mRootView.dragHandle(this, mCursorX);
                break;
            case MotionEvent.ACTION_MOVE:
                mDragging = mDragging || Math.hypot(ev.getRawX() - mDownRawX, ev.getRawY() - mDownRawY) >= mTouchSlop;
                int x = mDragging ? Math.round(ev.getRawX() - mPressedX) : mCursorX;
                // Also before the slop, so the menu stays hidden while a tap is undecided.
                mRootView.dragHandle(this, x);
                if (mDragging) {
                    SlintAndroidJavaHelper.moveCursorHandle(mId, x, Math.round(ev.getRawY() - mPressedY));
                }
                break;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_CANCEL:
                mRootView.endHandleDrag(this);
                // As in Android's own text fields, a tap on the cursor handle toggles the menu.
                if (ev.getActionMasked() == MotionEvent.ACTION_UP && !mDragging && mPopupWindow.isShowing()) {
                    mRootView.toggleActionMenu();
                }
                break;
        }
        return true;
    }

    public void setPosition(int x, int y) {
        mCursorX = x;
        mCursorY = y;
        x -= mOffsetX;

        if (mPopupWindow.isShowing()) {
            mPopupWindow.update(x, y, -1, -1);
        } else {
            mPopupWindow.showAtLocation(mRootView, Gravity.NO_GRAVITY, x, y);
        }
    }

    public int getCursorX() {
        return mCursorX;
    }

    public int getCursorY() {
        return mCursorY;
    }

    public void hide() {
        mPopupWindow.dismiss();
    }

    public void setHandleColor(int color) {
        // ImageView mutates the drawable before applying the filter, so other drawables
        // of the same resource keep their color.
        setColorFilter(color, PorterDuff.Mode.SRC_IN);
    }
}

class SlintInputView extends View {
    private boolean mAccessibilityClaimed = false;

    // The accessibility delegate can be installed once on a view: a second one panics in accesskit_android.
    synchronized boolean claimAccessibility() {
        boolean first = !mAccessibilityClaimed;
        mAccessibilityClaimed = true;
        return first;
    }

    // `dispatchHoverEvent` is protected, but the accessibility delegate listens for the events it dispatches.
    void passHoverEvent(MotionEvent event) {
        dispatchHoverEvent(event);
    }

    private int mInputType = EditorInfo.TYPE_CLASS_TEXT;
    private SlintEditable mEditable;
    private SlintInputConnection mInputConnection;
    private EditState mLastKnownSlintState;
    private boolean mRestartInputPending = false;
    // Whether a Slint text input is connected to the input method.
    private boolean mHasClient = false;

    private InputHandle mCursorHandle;
    private InputHandle mLeftHandle;
    private InputHandle mRightHandle;
    private Integer mHandleColor;
    private final Rect mSelectionRect = new Rect();
    // GameActivity draws into a SurfaceView, which a magnifier of this view doesn't show.
    private final boolean mMagnifierSupported;
    private Magnifier mMagnifier;
    private final int[] mMagnifierOrigin = new int[2];
    private InputHandle mDraggedHandle;
    private int mDragX;
    private int mCursorHeight;
    private int mNumHandles;
    private final Runnable mShowSelectionMenu = this::showSelectionMenu;
    private final Runnable mRefreshMagnifier = this::refreshMagnifier;
    private ActionMode mCurrentActionMode;
    private boolean mActionMenuDeferred = false;

    // The ids that callback_popup_menu_action in javahelper.rs expects.
    private static final int MENU_CUT = 0;
    private static final int MENU_COPY = 1;
    private static final int MENU_PASTE = 2;
    private static final int MENU_SELECT_ALL = 3;

    static final class EditState {
        final String text;
        final int selectionStart;
        final int selectionEnd;
        final int composingStart;
        final int composingEnd;

        EditState(String text, int selectionStart, int selectionEnd, int composingStart, int composingEnd) {
            this.text = text;
            this.selectionStart = selectionStart;
            this.selectionEnd = selectionEnd;
            boolean hasComposing = composingStart >= 0 && composingStart < composingEnd;
            this.composingStart = hasComposing ? composingStart : -1;
            this.composingEnd = hasComposing ? composingEnd : -1;
        }

        boolean hasComposing() {
            return composingStart >= 0;
        }

        boolean matches(SlintEditable e) {
            return e.toString().equals(text) && e.getSelectionStart() == selectionStart
                    && e.getSelectionEnd() == selectionEnd && e.getComposingStart() == composingStart
                    && e.getComposingEnd() == composingEnd;
        }
    }

    private class SlintEditable extends SpannableStringBuilder {
        private int mBatchEditNestDepth = 0;
        private String mTextWhenBeginBatchEdit;
        private int mSelectionStartWhenBeginBatchEdit;
        private int mSelectionEndWhenBeginBatchEdit;
        private int mComposingStartWhenBeginBatchEdit;
        private int mComposingEndWhenBeginBatchEdit;
        // Only used for setComposingRegion, which sets the span BaseInputConnection looks for.
        private final BaseInputConnection mDummyConnection;

        SlintEditable() {
            super();
            Editable self = this;
            mDummyConnection = new BaseInputConnection(SlintInputView.this, true) {
                @Override
                public Editable getEditable() {
                    return self;
                }
            };
        }

        void beginBatchEdit() {
            mBatchEditNestDepth++;
            if (mBatchEditNestDepth == 1) {
                mTextWhenBeginBatchEdit = toString();
                mSelectionStartWhenBeginBatchEdit = getSelectionStart();
                mSelectionEndWhenBeginBatchEdit = getSelectionEnd();
                mComposingStartWhenBeginBatchEdit = getComposingStart();
                mComposingEndWhenBeginBatchEdit = getComposingEnd();
            }
        }

        void endBatchEdit() {
            if (mBatchEditNestDepth == 0) {
                Log.e("slint", "endBatchEdit called without a matching beginBatchEdit");
                return;
            }
            if (mBatchEditNestDepth == 1) {
                boolean textChanged = !toString().equals(mTextWhenBeginBatchEdit);
                boolean selectionChanged = mSelectionStartWhenBeginBatchEdit != getSelectionStart()
                        || mSelectionEndWhenBeginBatchEdit != getSelectionEnd();
                boolean composingChanged = mComposingStartWhenBeginBatchEdit != getComposingStart()
                        || mComposingEndWhenBeginBatchEdit != getComposingEnd();
                notifyIfNeeded(textChanged, selectionChanged, composingChanged);
            }
            mBatchEditNestDepth--;
        }

        void setComposingRange(int start, int end) {
            if (start < 0 || start >= end) {
                BaseInputConnection.removeComposingSpans(this);
            } else {
                mDummyConnection.setComposingRegion(start, end);
            }
        }

        void setEditingState(EditState state) {
            beginBatchEdit();
            replace(0, length(), state.text);
            Selection.setSelection(this, state.selectionStart, state.selectionEnd);
            setComposingRange(state.composingStart, state.composingEnd);
            endBatchEdit();
        }

        @Override
        public SpannableStringBuilder replace(int start, int end, CharSequence tb, int tbstart, int tbend) {
            boolean textChanged = end - start != tbend - tbstart;
            for (int i = 0; i < end - start && !textChanged; i++) {
                textChanged |= charAt(start + i) != tb.charAt(tbstart + i);
            }
            int selectionStart = getSelectionStart();
            int selectionEnd = getSelectionEnd();
            int composingStart = getComposingStart();
            int composingEnd = getComposingEnd();

            super.replace(start, end, tb, tbstart, tbend);
            if (textChanged) {
                hideCursor();
            }

            if (mBatchEditNestDepth > 0) {
                return this;
            }
            boolean selectionChanged = getSelectionStart() != selectionStart || getSelectionEnd() != selectionEnd;
            boolean composingChanged = getComposingStart() != composingStart || getComposingEnd() != composingEnd;
            notifyIfNeeded(textChanged, selectionChanged, composingChanged);
            return this;
        }

        private void notifyIfNeeded(boolean textChanged, boolean selectionChanged, boolean composingChanged) {
            if (textChanged || selectionChanged || composingChanged) {
                didChangeEditingState(this);
            }
        }

        int getSelectionStart() {
            return Selection.getSelectionStart(this);
        }

        int getSelectionEnd() {
            return Selection.getSelectionEnd(this);
        }

        int getComposingStart() {
            return BaseInputConnection.getComposingSpanStart(this);
        }

        int getComposingEnd() {
            return BaseInputConnection.getComposingSpanEnd(this);
        }
    }

    private class SlintInputConnection extends BaseInputConnection {
        private final SlintEditable mConnectionEditable;
        private final InputMethodManager mImm;
        private ExtractedTextRequest mExtractRequest;
        private boolean mMonitorCursorUpdate = false;
        private CursorAnchorInfo.Builder mCursorAnchorInfoBuilder;
        private final ExtractedText mExtractedText = new ExtractedText();
        private int mBatchEditNestDepth = 0;

        SlintInputConnection() {
            super(SlintInputView.this, true);
            mConnectionEditable = mEditable;
            mImm = getContext().getSystemService(InputMethodManager.class);
        }

        private ExtractedText getExtractedText(ExtractedTextRequest request) {
            mExtractedText.startOffset = 0;
            mExtractedText.partialStartOffset = -1;
            mExtractedText.partialEndOffset = -1;
            mExtractedText.selectionStart = mConnectionEditable.getSelectionStart();
            mExtractedText.selectionEnd = mConnectionEditable.getSelectionEnd();
            mExtractedText.text = request == null || (request.flags & GET_TEXT_WITH_STYLES) == 0
                    ? mConnectionEditable.toString()
                    : mConnectionEditable;
            return mExtractedText;
        }

        private CursorAnchorInfo getCursorAnchorInfo() {
            if (mCursorAnchorInfoBuilder == null) {
                mCursorAnchorInfoBuilder = new CursorAnchorInfo.Builder();
            } else {
                mCursorAnchorInfoBuilder.reset();
            }
            mCursorAnchorInfoBuilder.setSelectionRange(mConnectionEditable.getSelectionStart(),
                    mConnectionEditable.getSelectionEnd());
            int composingStart = mConnectionEditable.getComposingStart();
            int composingEnd = mConnectionEditable.getComposingEnd();
            if (composingStart >= 0 && composingEnd > composingStart) {
                mCursorAnchorInfoBuilder.setComposingText(composingStart,
                        mConnectionEditable.toString().subSequence(composingStart, composingEnd));
            } else {
                mCursorAnchorInfoBuilder.setComposingText(-1, "");
            }
            return mCursorAnchorInfoBuilder.build();
        }

        @Override
        public Editable getEditable() {
            return mConnectionEditable;
        }

        @Override
        public boolean beginBatchEdit() {
            mConnectionEditable.beginBatchEdit();
            mBatchEditNestDepth += 1;
            return super.beginBatchEdit();
        }

        @Override
        public boolean endBatchEdit() {
            boolean result = super.endBatchEdit();
            mBatchEditNestDepth -= 1;
            mConnectionEditable.endBatchEdit();
            return result;
        }

        @Override
        public boolean deleteSurroundingText(int beforeLength, int afterLength) {
            if (mConnectionEditable.getSelectionStart() == -1) {
                return true;
            }
            return super.deleteSurroundingText(beforeLength, afterLength);
        }

        @Override
        public boolean setComposingText(CharSequence text, int newCursorPosition) {
            boolean result;
            beginBatchEdit();
            if (text.length() == 0) {
                result = super.commitText(text, newCursorPosition);
            } else {
                result = super.setComposingText(text, newCursorPosition);
            }
            endBatchEdit();
            return result;
        }

        // Returning null, as BaseInputConnection does, tells the input method that the editor can't
        // provide its text, and some then turn off suggestions and gesture typing.
        @Override
        public ExtractedText getExtractedText(ExtractedTextRequest request, int flags) {
            boolean textMonitor = (flags & GET_EXTRACTED_TEXT_MONITOR) != 0;
            mExtractRequest = textMonitor ? request : null;
            return getExtractedText(request);
        }

        @Override
        public boolean requestCursorUpdates(int cursorUpdateMode) {
            if ((cursorUpdateMode & CURSOR_UPDATE_IMMEDIATE) != 0) {
                mImm.updateCursorAnchorInfo(SlintInputView.this, getCursorAnchorInfo());
            }
            mMonitorCursorUpdate = (cursorUpdateMode & CURSOR_UPDATE_MONITOR) != 0;
            return true;
        }

        @Override
        public void closeConnection() {
            super.closeConnection();
            for (; mBatchEditNestDepth > 0; mBatchEditNestDepth--) {
                mConnectionEditable.endBatchEdit();
            }
            if (mInputConnection == this) {
                mInputConnection = null;
            }
        }

        @Override
        public boolean setSelection(int start, int end) {
            beginBatchEdit();
            boolean result = super.setSelection(start, end);
            endBatchEdit();
            return result;
        }

        // Through TextInput, like the floating menu, so that it applies its rules, such as not
        // copying passwords.
        @Override
        public boolean performContextMenuAction(int id) {
            int action;
            if (id == android.R.id.cut) {
                action = MENU_CUT;
            } else if (id == android.R.id.copy) {
                action = MENU_COPY;
            } else if (id == android.R.id.paste) {
                action = MENU_PASTE;
            } else if (id == android.R.id.selectAll) {
                action = MENU_SELECT_ALL;
            } else {
                return false;
            }
            SlintAndroidJavaHelper.popupMenuAction(action);
            return true;
        }

        // InputMethodManager.updateSelection skips the update if nothing changed since the last call.
        void didChangeEditingState() {
            mImm.updateSelection(SlintInputView.this, mConnectionEditable.getSelectionStart(),
                    mConnectionEditable.getSelectionEnd(), mConnectionEditable.getComposingStart(),
                    mConnectionEditable.getComposingEnd());
            if (mExtractRequest != null) {
                mImm.updateExtractedText(SlintInputView.this, mExtractRequest.token, getExtractedText(mExtractRequest));
            }
            if (mMonitorCursorUpdate) {
                mImm.updateCursorAnchorInfo(SlintInputView.this, getCursorAnchorInfo());
            }
        }
    }

    public SlintInputView(Context context) {
        super(context);
        setFocusable(true);
        setFocusableInTouchMode(true);
        mEditable = new SlintEditable();
        mMagnifierSupported = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && context instanceof NativeActivity;
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        if (!mHasClient) {
            mInputConnection = null;
            return null;
        }
        outAttrs.inputType = mInputType;
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI;
        mInputConnection = new SlintInputConnection();
        outAttrs.initialSelStart = mEditable.getSelectionStart();
        outAttrs.initialSelEnd = mEditable.getSelectionEnd();
        return mInputConnection;
    }

    private void didChangeEditingState(SlintEditable editable) {
        if (editable != mEditable || !mHasClient) {
            return;
        }
        if (mLastKnownSlintState != null && !mLastKnownSlintState.matches(editable)) {
            mLastKnownSlintState = new EditState(editable.toString(), editable.getSelectionStart(),
                    editable.getSelectionEnd(), editable.getComposingStart(), editable.getComposingEnd());
            SlintAndroidJavaHelper.updateText(mLastKnownSlintState.text, mLastKnownSlintState.selectionEnd,
                    mLastKnownSlintState.selectionStart, mLastKnownSlintState.composingStart,
                    mLastKnownSlintState.composingEnd);
        }
        if (mInputConnection != null && mInputConnection.mConnectionEditable == editable) {
            mInputConnection.didChangeEditingState();
        }
    }

    public void clearTextInputClient() {
        mHasClient = false;
        mLastKnownSlintState = null;
        // Some keyboards cache the old state, so restart unless the keyboard is still showing.
        WindowInsets insets = getRootWindowInsets();
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R || insets == null
                || !insets.isVisible(WindowInsets.Type.ime())) {
            getContext().getSystemService(InputMethodManager.class).restartInput(this);
        }
    }

    public void setTextInputClient(int inputType) {
        mInputType = inputType;
        mHasClient = true;
        mEditable = new SlintEditable();
        mLastKnownSlintState = null;
        mRestartInputPending = true;
    }

    public void setTextInputEditingState(EditState state) {
        if (!mRestartInputPending && mLastKnownSlintState != null && mLastKnownSlintState.hasComposing()) {
            // Input methods don't expect the editor to change the composition, and only reset their
            // state on a restart.
            mRestartInputPending = composingChanged(mLastKnownSlintState, state);
        }
        mLastKnownSlintState = state;
        mEditable.setEditingState(state);
        if (mRestartInputPending) {
            getContext().getSystemService(InputMethodManager.class).restartInput(this);
            mRestartInputPending = false;
        }
    }

    private static boolean composingChanged(EditState before, EditState after) {
        int composingLength = before.composingEnd - before.composingStart;
        if (composingLength != after.composingEnd - after.composingStart) {
            return true;
        }
        for (int i = 0; i < composingLength; i++) {
            if (before.text.charAt(i + before.composingStart) != after.text.charAt(i + after.composingStart)) {
                return true;
            }
        }
        return false;
    }

    @Override
    protected void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        SlintAndroidJavaHelper.setNightMode(newConfig.uiMode & Configuration.UI_MODE_NIGHT_MASK);
        SlintAndroidJavaHelper.setFontScale(newConfig.fontScale);
    }

    // As in Android's own text fields, the menu hides while a handle is held
    // and comes back a moment after it is released.
    public void dragHandle(InputHandle handle, int x) {
        if (mDraggedHandle == null) {
            // Android's cursor doesn't blink while a handle is held.
            SlintAndroidJavaHelper.setCursorBlinkSuspended(true);
        }
        mDraggedHandle = handle;
        mDragX = x;
        removeCallbacks(mShowSelectionMenu);
        if (mCurrentActionMode != null) {
            mCurrentActionMode.hide(ActionMode.DEFAULT_HIDE_DURATION);
        }
        updateMagnifier();
    }

    public void endHandleDrag(InputHandle handle) {
        if (handle != mDraggedHandle) {
            return;
        }
        mDraggedHandle = null;
        SlintAndroidJavaHelper.setCursorBlinkSuspended(false);
        if (mMagnifier != null) {
            mMagnifier.dismiss();
            // Built again for the next drag, so it picks up a changed screen density.
            mMagnifier = null;
        }
        postDelayed(mShowSelectionMenu, ViewConfiguration.getDoubleTapTimeout());
    }

    public void toggleActionMenu() {
        if (mCurrentActionMode != null) {
            finishActionMenu();
        } else {
            showActionMenu();
        }
    }

    private void showSelectionMenu() {
        if ((mNumHandles == 2 || mCurrentActionMode != null) && !mActionMenuDeferred) {
            showActionMenu();
        }
    }

    private void updateMagnifier() {
        if (mDraggedHandle == null || !mMagnifierSupported) {
            return;
        }
        if (mMagnifier == null) {
            // TextView's magnifier differs from Magnifier.Builder's defaults in these values.
            TypedArray a = new ContextThemeWrapper(getContext(), android.R.style.Theme_DeviceDefault)
                    .obtainStyledAttributes(new int[] { android.R.attr.dialogCornerRadius });
            float cornerRadius = a.getDimension(0, 0);
            a.recycle();
            mMagnifier = new Magnifier.Builder(this)
                    .setCornerRadius(cornerRadius)
                    .setOverlay(new ColorDrawable(0x0EFFFFFF))
                    .setSourceBounds(Magnifier.SOURCE_BOUND_MAX_VISIBLE, Magnifier.SOURCE_BOUND_MAX_IN_SURFACE,
                            Magnifier.SOURCE_BOUND_MAX_VISIBLE, Magnifier.SOURCE_BOUND_MAX_IN_SURFACE)
                    .build();
            removeCallbacks(mRefreshMagnifier);
            postOnAnimation(mRefreshMagnifier);
        }
        float halfLine = mCursorHeight / 2f;
        float cursorX = mDraggedHandle.getCursorX();
        // Within half a line height of the cursor, so the magnifier stops at the ends of the text.
        float x = Math.max(cursorX - halfLine, Math.min(mDragX, cursorX + halfLine));
        getLocationInWindow(mMagnifierOrigin);
        mMagnifier.show(x - mMagnifierOrigin[0], mDraggedHandle.getCursorY() - halfLine - mMagnifierOrigin[1]);
    }

    // The magnifier copies the screen only when asked, and Slint draws after the cursor moved,
    // so copy every frame while it shows.
    private void refreshMagnifier() {
        if (mMagnifier != null) {
            mMagnifier.update();
            postOnAnimation(mRefreshMagnifier);
        }
    }

    private InputHandle updateHandle(InputHandle handle, int id, int x, int y) {
        if (x == -1) {
            hideHandle(handle);
            return handle;
        }
        if (handle == null) {
            handle = new InputHandle(this, id);
            if (mHandleColor != null) {
                handle.setHandleColor(mHandleColor);
            }
        }
        handle.setPosition(x, y);
        return handle;
    }

    private void hideHandle(InputHandle handle) {
        if (handle != null) {
            endHandleDrag(handle);
            handle.hide();
        }
    }

    public void hideCursor() {
        setCursorPos(0, 0, 0, 0, 0, 0);
    }

    // numHandles: 0=hidden, 1=cursor handle, 2=selection handles
    public void setCursorPos(int leftX, int leftY, int rightX, int rightY, int cursorHeight, int numHandles) {
        int handleHeight = 0;
        mCursorHeight = cursorHeight;
        mNumHandles = numHandles;
        if (numHandles == 1) {
            hideHandle(mLeftHandle);
            hideHandle(mRightHandle);
            mCursorHandle = updateHandle(mCursorHandle, InputHandle.CURSOR, leftX, leftY);
            if (leftX != -1) {
                handleHeight = mCursorHandle.getHeight();
            }
        } else if (numHandles == 2) {
            hideHandle(mCursorHandle);
            mLeftHandle = updateHandle(mLeftHandle, InputHandle.SELECTION_START, leftX, leftY);
            if (leftX != -1) {
                handleHeight = mLeftHandle.getHeight();
            }
            mRightHandle = updateHandle(mRightHandle, InputHandle.SELECTION_END, rightX, rightY);
            if (rightX != -1) {
                handleHeight = mRightHandle.getHeight();
            }
            if (!mActionMenuDeferred) {
                showActionMenu();
            }
        } else {
            if (mCursorHandle != null) {
                handleHeight = mCursorHandle.getHeight();
            }
            hideHandle(mCursorHandle);
            hideHandle(mLeftHandle);
            hideHandle(mRightHandle);
            finishActionMenu();
        }

        updateMagnifier();

        mSelectionRect.set(Math.min(leftX, rightX), Math.min(leftY, rightY) - cursorHeight,
                Math.max(leftX, rightX), Math.max(leftY, rightY) + handleHeight);
        if (mCurrentActionMode != null) {
            mCurrentActionMode.invalidateContentRect();
        }
    }

    public void setHandleColor(int color) {
        mHandleColor = color;
        for (InputHandle handle : new InputHandle[] { mCursorHandle, mLeftHandle, mRightHandle }) {
            if (handle != null) {
                handle.setHandleColor(color);
            }
        }
    }

    public void showActionMenu() {
        if (mDraggedHandle != null) {
            return;
        }
        if (mCurrentActionMode != null) {
            mCurrentActionMode.invalidate();
            if (mCurrentActionMode.getMenu().hasVisibleItems()) {
                mCurrentActionMode.hide(0);
            } else {
                finishActionMenu();
            }
            return;
        }
        ActionMode.Callback2 action = new ActionMode.Callback2() {
            @Override
            public boolean onCreateActionMode(ActionMode mode, Menu menu) {
                mode.setTitle(null);
                mode.setSubtitle(null);
                mode.setTitleOptionalHint(true);
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    menu.setGroupDividerEnabled(true);
                }

                final TypedArray a = getContext().obtainStyledAttributes(new int[] {
                        android.R.attr.actionModeCutDrawable,
                        android.R.attr.actionModeCopyDrawable,
                        android.R.attr.actionModePasteDrawable,
                        android.R.attr.actionModeSelectAllDrawable,
                });

                menu.add(Menu.FIRST, MENU_CUT, 0, android.R.string.cut)
                        .setAlphabeticShortcut('x')
                        .setIcon(a.getDrawable(0));
                menu.add(Menu.FIRST, MENU_COPY, 1, android.R.string.copy)
                        .setAlphabeticShortcut('c')
                        .setIcon(a.getDrawable(1));
                menu.add(Menu.FIRST, MENU_PASTE, 2, android.R.string.paste)
                        .setAlphabeticShortcut('v')
                        .setIcon(a.getDrawable(2));
                menu.add(Menu.FIRST, MENU_SELECT_ALL, 3, android.R.string.selectAll)
                        .setAlphabeticShortcut('a')
                        .setIcon(a.getDrawable(3));

                a.recycle();

                // Like Android's Editor, an empty menu doesn't start, or it would swallow the next tap.
                updateMenuItems(menu);
                return menu.hasVisibleItems();
            }

            @Override
            public boolean onPrepareActionMode(ActionMode mode, Menu menu) {
                updateMenuItems(menu);
                return true;
            }

            @Override
            public boolean onActionItemClicked(ActionMode mode, MenuItem item) {
                SlintAndroidJavaHelper.popupMenuAction(item.getItemId());
                mode.finish();
                return true;
            }

            @Override
            public void onDestroyActionMode(ActionMode action) {
                mCurrentActionMode = null;
            }

            // Introduced in API level 23
            @Override
            public void onGetContentRect(ActionMode mode, View view, Rect outRect) {
                // The selection rect is in window coordinates, Android expects it relative to the
                // view, which sits below the status bar unless the window is edge-to-edge.
                int[] location = new int[2];
                view.getLocationInWindow(location);
                outRect.set(mSelectionRect);
                outRect.offset(-location[0], -location[1]);
            }
        };
        mCurrentActionMode = startActionMode(action, ActionMode.TYPE_FLOATING);
    }

    private void updateMenuItems(Menu menu) {
        int selectionStart = Selection.getSelectionStart(mEditable);
        int selectionEnd = Selection.getSelectionEnd(mEditable);
        int start = Math.min(selectionStart, selectionEnd);
        int end = Math.max(selectionStart, selectionEnd);
        boolean hasSelection = start >= 0 && start != end;
        boolean editable = (mInputType & EditorInfo.TYPE_MASK_CLASS) != EditorInfo.TYPE_NULL;
        boolean password = (mInputType & EditorInfo.TYPE_MASK_VARIATION)
                == EditorInfo.TYPE_TEXT_VARIATION_PASSWORD;
        ClipboardManager clipboard = getContext().getSystemService(ClipboardManager.class);
        menu.findItem(MENU_CUT).setVisible(hasSelection && editable && !password);
        menu.findItem(MENU_COPY).setVisible(hasSelection && !password);
        menu.findItem(MENU_PASTE).setVisible(editable && clipboard.hasPrimaryClip());
        menu.findItem(MENU_SELECT_ALL).setVisible(start > 0 || end < mEditable.length());
    }

    void deferActionMenu(boolean deferred) {
        mActionMenuDeferred = deferred;
        if (deferred) {
            finishActionMenu();
        }
    }

    void finishActionMenu() {
        removeCallbacks(mShowSelectionMenu);
        if (mCurrentActionMode != null) {
            mCurrentActionMode.finish();
            mCurrentActionMode = null;
        }
    }
}

// Draws the focus indicator of accessibility services around the node that has accessibility focus.
//
// Android draws the indicator in the window of the activity, which doesn't happen for a native
// activity: it renders to the surface of the window itself, so its views are never drawn. That is
// why the indicator is drawn in a window of its own, on top. A GameActivity doesn't need this.
class AccessibilityFocusFrame extends View {
    // The look of the system, when the accessibility settings don't tell.
    private static final int DEFAULT_COLOR = 0xFF4DAA40;
    private static final int DEFAULT_STROKE_DP = 4;

    private final SlintInputView mHost;
    private final PopupWindow mWindow;
    private final Paint mPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Rect mBounds = new Rect();
    private final Runnable mUpdate = this::update;
    private final View.OnLayoutChangeListener mLayoutListener =
            (v, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom) -> scheduleUpdate();
    private ViewGroup mParent;
    private View.AccessibilityDelegate mPreviousDelegate;
    private boolean mUpdatePending;
    private boolean mDestroyed;

    AccessibilityFocusFrame(Context context, SlintInputView host) {
        super(context);
        mHost = host;
        mPaint.setStyle(Paint.Style.STROKE);
        // A service should only see the nodes of the application, not the frame.
        setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        mWindow = new PopupWindow(this, 1, 1);
        mWindow.setTouchable(false);
        mWindow.setFocusable(false);
        mWindow.setClippingEnabled(false);
        mWindow.setElevation(0);
    }

    // The events for the accessibility services tell when the focus or a node moved.
    void install() {
        mParent = (ViewGroup) mHost.getParent();
        if (mParent == null) {
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            mPreviousDelegate = mParent.getAccessibilityDelegate();
        }
        mParent.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override
            public boolean onRequestSendAccessibilityEvent(ViewGroup group, View child, AccessibilityEvent event) {
                if (child == mHost) {
                    switch (event.getEventType()) {
                        case AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED:
                        case AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED:
                        case AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED:
                        case AccessibilityEvent.TYPE_VIEW_SCROLLED:
                            scheduleUpdate();
                            break;
                        default:
                            break;
                    }
                }
                if (mPreviousDelegate != null) {
                    return mPreviousDelegate.onRequestSendAccessibilityEvent(group, child, event);
                }
                return super.onRequestSendAccessibilityEvent(group, child, event);
            }
        });
    }

    void scheduleUpdate() {
        if (!mUpdatePending && !mDestroyed) {
            mUpdatePending = true;
            mHost.post(mUpdate);
        }
    }

    // Shows the frame around the node with accessibility focus, if there is one.
    private void update() {
        mUpdatePending = false;
        if (mDestroyed) {
            return;
        }
        AccessibilityManager manager = (AccessibilityManager) getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
        AccessibilityNodeProvider provider = mHost.getAccessibilityNodeProvider();
        AccessibilityNodeInfo node = null;
        if (manager != null && manager.isEnabled() && provider != null && mHost.isAttachedToWindow()) {
            node = provider.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY);
        }
        if (node == null) {
            dismiss();
            return;
        }
        node.getBoundsInScreen(mBounds);
        if (mBounds.isEmpty()) {
            dismiss();
            return;
        }
        int color = DEFAULT_COLOR;
        float strokeWidth = DEFAULT_STROKE_DP * getResources().getDisplayMetrics().density;
        // The accessibility settings of API level 35. Looked up when needed, so that this compiles
        // with an older android.jar too.
        if (Build.VERSION.SDK_INT >= 35) {
            try {
                color = (Integer) AccessibilityManager.class.getMethod("getAccessibilityFocusColor").invoke(manager);
                strokeWidth = (Integer) AccessibilityManager.class.getMethod("getAccessibilityFocusStrokeWidth")
                        .invoke(manager);
            } catch (ReflectiveOperationException e) {
                // Keep the defaults.
            }
        }
        mPaint.setColor(color);
        mPaint.setStrokeWidth(strokeWidth);
        // The window of the frame is placed relative to the window of the activity.
        int[] onScreen = new int[2];
        int[] inWindow = new int[2];
        mHost.getLocationOnScreen(onScreen);
        mHost.getLocationInWindow(inWindow);
        int x = mBounds.left - (onScreen[0] - inWindow[0]);
        int y = mBounds.top - (onScreen[1] - inWindow[1]);
        try {
            if (mWindow.isShowing()) {
                mWindow.update(x, y, mBounds.width(), mBounds.height());
            } else {
                mWindow.setWidth(mBounds.width());
                mWindow.setHeight(mBounds.height());
                mWindow.showAtLocation(mHost, Gravity.TOP | Gravity.START, x, y);
                // Resizing or moving the window of the activity moves the node without an event.
                mHost.addOnLayoutChangeListener(mLayoutListener);
            }
        } catch (WindowManager.BadTokenException e) {
            // The activity is going away.
            return;
        }
        invalidate();
    }

    void dismiss() {
        if (mWindow.isShowing()) {
            mWindow.dismiss();
            mHost.removeOnLayoutChangeListener(mLayoutListener);
        }
    }

    // The window of the activity changed: the frame has to follow, if it is showing.
    void windowChanged() {
        if (mWindow.isShowing()) {
            scheduleUpdate();
        }
    }

    // No event tells that the last service was turned off, so the frame goes away here.
    void accessibilityStateChanged(boolean enabled) {
        if (!enabled) {
            dismiss();
        }
    }

    void destroy() {
        mDestroyed = true;
        mHost.removeCallbacks(mUpdate);
        dismiss();
        if (mParent != null) {
            mParent.setAccessibilityDelegate(mPreviousDelegate);
            mParent = null;
        }
    }

    @Override
    protected void onDraw(Canvas canvas) {
        float half = mPaint.getStrokeWidth() / 2;
        float radius = getResources().getDisplayMetrics().density * 2;
        canvas.drawRoundRect(half, half, getWidth() - half, getHeight() - half, radius, radius, mPaint);
    }
}

public class SlintAndroidJavaHelper {
    private final Activity mActivity;
    private final SlintInputView mInputView;
    private AccessibilityFocusFrame mFocusFrame;

    public SlintAndroidJavaHelper(Activity activity) {
        mActivity = activity;
        mInputView = new SlintInputView(activity);
        mActivity.runOnUiThread(this::attachInputView);
    }

    private void attachInputView() {
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT);
        mActivity.addContentView(mInputView, params);
        View rootView = rootView();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            rootView.setOnApplyWindowInsetsListener((v, insets) -> dispatchInsets(insets));
            // Attach the IME animation callback to the input view rather than the
            // decor root: some OEM ROMs fail to render the IME surface when an
            // animation callback is installed on the window's root view.
            mInputView.setWindowInsetsAnimationCallback(
                    new WindowInsetsAnimation.Callback(
                            WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
                        @Override
                        public WindowInsets onProgress(WindowInsets insets,
                                List<WindowInsetsAnimation> runningAnimations) {
                            return dispatchInsets(insets);
                        }
                    });
        } else {
            rootView.getViewTreeObserver().addOnGlobalLayoutListener(this::dispatchLayoutInsets);
        }
        listenToAccessibilityState();
        // On API 34+, Back arrives via OnBackInvokedDispatcher; forward
        // it into Slint's key-event pipeline.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            mActivity.getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT, SlintAndroidJavaHelper::onBackInvoked);
        }
    }

    // The accessibility manager keeps its listeners alive, so they are removed with the activity.
    private void listenToAccessibilityState() {
        AccessibilityManager manager =
                (AccessibilityManager) mActivity.getSystemService(Context.ACCESSIBILITY_SERVICE);
        if (manager == null) {
            return;
        }
        AccessibilityManager.TouchExplorationStateChangeListener touchListener =
                enabled -> accessibilityStateChanged(manager.isEnabled(), enabled);
        AccessibilityManager.AccessibilityStateChangeListener stateListener = enabled -> {
            if (mFocusFrame != null) {
                mFocusFrame.accessibilityStateChanged(enabled);
            }
            accessibilityStateChanged(enabled, manager.isTouchExplorationEnabled());
        };
        manager.addTouchExplorationStateChangeListener(touchListener);
        manager.addAccessibilityStateChangeListener(stateListener);
        accessibilityStateChanged(manager.isEnabled(), manager.isTouchExplorationEnabled());
        Application application = mActivity.getApplication();
        application.registerActivityLifecycleCallbacks(new Application.ActivityLifecycleCallbacks() {
            @Override
            public void onActivityDestroyed(Activity activity) {
                if (activity == mActivity) {
                    manager.removeTouchExplorationStateChangeListener(touchListener);
                    manager.removeAccessibilityStateChangeListener(stateListener);
                    if (mFocusFrame != null) {
                        mFocusFrame.destroy();
                    }
                    application.unregisterActivityLifecycleCallbacks(this);
                }
            }

            @Override
            public void onActivityCreated(Activity activity, Bundle savedInstanceState) {}

            @Override
            public void onActivityStarted(Activity activity) {}

            @Override
            public void onActivityResumed(Activity activity) {}

            @Override
            public void onActivityPaused(Activity activity) {}

            @Override
            public void onActivityStopped(Activity activity) {}

            @Override
            public void onActivitySaveInstanceState(Activity activity, Bundle outState) {}
        });
    }

    private View rootView() {
        return mActivity.getWindow().getDecorView().getRootView();
    }

    private static void sendInsets(Rect window, Rect safeArea, Rect keyboard) {
        setInsets(
                window.top, window.left, window.bottom, window.right,
                safeArea.top, safeArea.left, safeArea.bottom, safeArea.right,
                keyboard.top, keyboard.left, keyboard.bottom, keyboard.right);
    }

    private static Rect safeArea(WindowInsets insets) {
        Insets safeArea = Insets.max(
                insets.getInsets(WindowInsets.Type.systemBars()),
                insets.getInsets(WindowInsets.Type.displayCutout()));
        return new Rect(safeArea.left, safeArea.top, safeArea.right, safeArea.bottom);
    }

    private WindowInsets dispatchInsets(WindowInsets insets) {
        if (mFocusFrame != null) {
            mFocusFrame.windowChanged();
        }
        // The listener-supplied `insets` reflects what reaches the decor view
        // AFTER any ancestor has consumed insets, so in edge-to-edge mode the
        // system bars and the display cutout often arrive as zero. Read those
        // from the WindowManager instead, which always returns the unconsumed
        // values. The IME inset still comes from the listener stream so
        // keyboard show/hide animates.
        WindowMetrics metrics = mActivity.getWindowManager().getCurrentWindowMetrics();
        Insets keyboard = insets.getInsets(WindowInsets.Type.ime());
        sendInsets(metrics.getBounds(), safeArea(metrics.getWindowInsets()),
                new Rect(keyboard.left, keyboard.top, keyboard.right, keyboard.bottom));
        return insets;
    }

    private void dispatchLayoutInsets() {
        Rect windowRect = get_view_rect();
        Rect safeArea = get_safe_area();

        // This is only an approximation, because SDK level < 30 doesn't provide
        // a way to get the keyboard area directly.
        Rect visibleRect = new Rect();
        rootView().getWindowVisibleDisplayFrame(visibleRect);
        int top = windowRect.top - visibleRect.top;
        int left = windowRect.left - visibleRect.left;
        int bottom = windowRect.bottom - visibleRect.bottom;
        int right = windowRect.right - visibleRect.right;

        // only take the largest value (it's probably always going to be bottom)
        Rect keyboard = new Rect();
        if (bottom >= Math.max(left, Math.max(top, right))) {
            keyboard.bottom = bottom;
        } else if (left >= Math.max(top, right)) {
            keyboard.left = left;
        } else if (top >= right) {
            keyboard.top = top;
        } else {
            keyboard.right = right;
        }

        sendInsets(windowRect, safeArea, keyboard);
    }

    public void show_keyboard() {
        mActivity.runOnUiThread(() -> {
            mInputView.requestFocus();
            mActivity.getSystemService(InputMethodManager.class).showSoftInput(mInputView, 0);
        });
    }

    public void hide_keyboard() {
        mActivity.runOnUiThread(() -> {
            mActivity.getSystemService(InputMethodManager.class)
                    .hideSoftInputFromWindow(mInputView.getWindowToken(), 0);
            mInputView.clearFocus();
            mInputView.hideCursor();
        });
    }

    // Called from Rust when an OnBackInvokedCallback fires and Slint's key
    // dispatch reports the Back key as unhandled — preserves the legacy
    // Back-closes-the-activity default.
    public void finish_activity() {
        mActivity.runOnUiThread(mActivity::finish);
    }

    public static native void updateText(String text, int cursorPosition, int anchorPosition, int preeditStart,
            int preeditEnd);

    public static native void setNightMode(int nightMode);

    public static native void setFontScale(float fontScale);

    public static native void onBackInvoked();

    public static native void accessibilityStateChanged(boolean enabled, boolean touchExploration);

    public static native void moveCursorHandle(int id, int posX, int posY);

    public static native void setCursorBlinkSuspended(boolean suspended);

    public static native void popupMenuAction(int id);

    public static native void setInsets(int windowTop, int windowLeft, int windowBottom, int windowRight,
            int safeAreaTop, int safeAreaLeft, int safeAreaBottom, int safeAreaRight,
            int keyboardTop, int keyboardLeft, int keyboardBottom, int keyboardRight);

    public void set_imm_data(String text, int cursorPosition, int anchorPosition, int preeditStart, int preeditEnd,
            int curX, int curY, int anchorX, int anchorY, int cursorHeight, int inputType,
            boolean showCursorHandles, boolean newClient, boolean updateEditingState) {
        mActivity.runOnUiThread(() -> {
            if (newClient) {
                mInputView.setTextInputClient(inputType);
            }
            if (updateEditingState) {
                // Android's selection start is the anchor, and its end is the cursor.
                mInputView.setTextInputEditingState(
                        new SlintInputView.EditState(text, anchorPosition, cursorPosition, preeditStart, preeditEnd));
            }
            int numHandles = 0;
            if (showCursorHandles) {
                numHandles = cursorPosition == anchorPosition ? 1 : 2;
            }
            if (cursorPosition < anchorPosition) {
                mInputView.setCursorPos(curX, curY, anchorX, anchorY, cursorHeight, numHandles);
            } else {
                mInputView.setCursorPos(anchorX, anchorY, curX, curY, cursorHeight, numHandles);
            }
        });
    }

    public void set_handle_color(int color) {
        mActivity.runOnUiThread(() -> mInputView.setHandleColor(color));
    }

    // Uses dark system bar icons when `light` is true, light ones otherwise.
    public void set_light_system_bars(boolean light) {
        mActivity.runOnUiThread(() -> {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                int mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                        | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                mActivity.getWindow().getInsetsController().setSystemBarsAppearance(light ? mask : 0, mask);
                return;
            }
            int flags = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                flags |= View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
            }
            View decorView = mActivity.getWindow().getDecorView();
            int visibility = decorView.getSystemUiVisibility();
            decorView.setSystemUiVisibility(light ? visibility | flags : visibility & ~flags);
        });
    }

    public int color_scheme() {
        return mActivity.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK;
    }

    public float font_scale() {
        return mActivity.getResources().getConfiguration().fontScale;
    }

    public float horizontal_scroll_factor() {
        return ViewConfiguration.get(mActivity).getScaledHorizontalScrollFactor();
    }

    public float vertical_scroll_factor() {
        return ViewConfiguration.get(mActivity).getScaledVerticalScrollFactor();
    }

    public boolean animators_enabled() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            return ValueAnimator.areAnimatorsEnabled();
        }
        return Settings.Global.getFloat(mActivity.getContentResolver(), Settings.Global.ANIMATOR_DURATION_SCALE, 1.0f) != 0;
    }

    public int accent_color() {
        TypedArray a = mActivity.getTheme().obtainStyledAttributes(new int[] { android.R.attr.colorAccent });
        int color = a.getColor(0, 0);
        a.recycle();
        return color;
    }

    // The view that covers the content of the activity. The accessibility delegate is installed on it.
    public boolean claim_accessibility_host() {
        boolean claimed = mInputView.claimAccessibility();
        // A GameActivity draws the focus indicator of accessibility services itself.
        if (claimed && mActivity instanceof NativeActivity) {
            mActivity.runOnUiThread(() -> {
                mFocusFrame = new AccessibilityFocusFrame(mActivity, mInputView);
                mFocusFrame.install();
            });
        }
        return claimed;
    }

    public View get_input_view() {
        return mInputView;
    }

    // The origin of the window relative to the input view, both on screen. AccessKit expects the
    // bounds of the nodes relative to the view that hosts the accessibility delegate.
    public Rect get_window_origin_in_view() {
        int[] view = new int[2];
        mInputView.getLocationOnScreen(view);
        int[] window = new int[2];
        rootView().getLocationOnScreen(window);
        return new Rect(window[0] - view[0], window[1] - view[1], 0, 0);
    }

    // Hover events, which touch exploration is made of, arrive on the native input queue and never
    // reach the view hierarchy of a native activity. Pass them on to the input view, where the
    // accessibility delegate listens for them.
    public void dispatch_hover_event(int action, float x, float y) {
        mActivity.runOnUiThread(() -> {
            int[] origin = new int[2];
            mInputView.getLocationInWindow(origin);
            long now = SystemClock.uptimeMillis();
            MotionEvent event = MotionEvent.obtain(now, now, action, x - origin[0], y - origin[1], 0);
            event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            mInputView.passHoverEvent(event);
            event.recycle();
        });
    }

    // Get the size of the window
    public Rect get_view_rect() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // On Android 11 and above, we can get the window bounds directly
            return mActivity.getWindowManager().getCurrentWindowMetrics().getBounds();
        }
        View rootView = rootView();
        return new Rect(rootView.getLeft(), rootView.getTop(), rootView.getRight(), rootView.getBottom());
    }

    // On SDK level < 30, returns the inset for the safe area and the keyboard.
    // On SDK level >= 30, returns the inset for the safe area only.
    public Rect get_safe_area() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            return safeArea(mActivity.getWindowManager().getCurrentWindowMetrics().getWindowInsets());
        }
        // Note: `View.getRootWindowInsets` requires API level 23 or above
        WindowInsets insets = rootView().getRootWindowInsets();
        if (insets == null) {
            return new Rect();
        }
        return new Rect(
                insets.getStableInsetLeft(),
                insets.getStableInsetTop(),
                insets.getStableInsetRight(),
                insets.getStableInsetBottom());
    }

    public void show_action_menu() {
        mActivity.runOnUiThread(mInputView::showActionMenu);
    }

    public void defer_action_menu(boolean deferred) {
        mActivity.runOnUiThread(() -> mInputView.deferActionMenu(deferred));
    }

    public void hide_action_menu() {
        mActivity.runOnUiThread(mInputView::finishActionMenu);
    }

    public void clear_text_input_client() {
        mActivity.runOnUiThread(mInputView::clearTextInputClient);
    }

    public String get_clipboard() {
        FutureTask<String> future = new FutureTask<>(() -> {
            ClipData clip = mActivity.getSystemService(ClipboardManager.class).getPrimaryClip();
            if (clip == null || clip.getItemCount() == 0) {
                return null;
            }
            CharSequence text = clip.getItemAt(0).coerceToText(mActivity);
            return text == null ? null : text.toString();
        });

        mActivity.runOnUiThread(future);
        try {
            return future.get();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return null;
        } catch (ExecutionException e) {
            Log.w("slint", "Failed to read the clipboard", e.getCause());
            return null;
        }
    }

    public void set_clipboard(String text) {
        mActivity.runOnUiThread(() -> mActivity.getSystemService(ClipboardManager.class)
                .setPrimaryClip(ClipData.newPlainText(null, text)));
    }
}
