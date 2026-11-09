// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore drawables javahelper Spannable tbstart tbend

package dev.slint.android;

import android.animation.ValueAnimator;
import android.app.Activity;
import android.app.NativeActivity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.Configuration;
import android.content.res.TypedArray;
import android.graphics.Insets;
import android.graphics.PorterDuff;
import android.graphics.Rect;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.Drawable;
import android.os.Build;
import android.provider.Settings;
import android.text.Editable;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.util.Log;
import android.view.ActionMode;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.Menu;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.WindowInsetsController;
import android.view.WindowMetrics;
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
                mDragging = mId != CURSOR;
                mRootView.dragHandle(this, mCursorX);
                break;
            case MotionEvent.ACTION_MOVE:
                if (!mDragging) {
                    mDragging = Math.hypot(ev.getRawX() - mDownRawX, ev.getRawY() - mDownRawY) >= mTouchSlop;
                    if (!mDragging) {
                        // Keeps the menu hidden, which comes back on its own after a while.
                        mRootView.dragHandle(this, mCursorX);
                        break;
                    }
                }
                int x = Math.round(ev.getRawX() - mPressedX);
                mRootView.dragHandle(this, x);
                SlintAndroidJavaHelper.moveCursorHandle(mId, x, Math.round(ev.getRawY() - mPressedY));
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

                // As in Android's own text fields, a menu without items doesn't start, so it
                // can't take the next tap that would show it.
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

public class SlintAndroidJavaHelper {
    private final Activity mActivity;
    private final SlintInputView mInputView;

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
        // On API 34+, Back arrives via OnBackInvokedDispatcher; forward
        // it into Slint's key-event pipeline.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            mActivity.getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT, SlintAndroidJavaHelper::onBackInvoked);
        }
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
