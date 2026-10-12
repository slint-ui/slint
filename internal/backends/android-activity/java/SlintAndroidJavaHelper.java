// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore drawables javahelper Spannable tbstart tbend

package dev.slint.android;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.Configuration;
import android.content.res.TypedArray;
import android.graphics.Insets;
import android.graphics.PorterDuff;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;
import android.os.Build;
import android.text.Editable;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.util.Log;
import android.view.ActionMode;
import android.view.Gravity;
import android.view.Menu;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.WindowInsetsController;
import android.view.WindowMetrics;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;
import android.widget.ImageView;
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
    private int mCursorX;
    private int mCursorY;
    private float mPressedX;
    private float mPressedY;

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
                break;
            case MotionEvent.ACTION_MOVE:
                mRootView.finishActionMenu();
                SlintAndroidJavaHelper.moveCursorHandle(mId, Math.round(ev.getRawX() - mPressedX),
                        Math.round(ev.getRawY() - mPressedY));
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
    private String mText = "";
    private int mCursorPosition = 0;
    private int mAnchorPosition = 0;
    private int mInputType = EditorInfo.TYPE_CLASS_TEXT;
    private int mInBatch = 0;
    private boolean mPending = false;
    private SlintEditable mEditable;

    private InputHandle mCursorHandle;
    private InputHandle mLeftHandle;
    private InputHandle mRightHandle;
    private Integer mHandleColor;
    private final Rect mSelectionRect = new Rect();
    private ActionMode mCurrentActionMode;

    private class SlintEditable extends SpannableStringBuilder {
        public SlintEditable() {
            super(mText);
        }

        @Override
        public SpannableStringBuilder replace(int start, int end, CharSequence tb, int tbstart, int tbend) {
            super.replace(start, end, tb, tbstart, tbend);
            hideCursor();
            if (mInBatch == 0) {
                update();
            } else {
                mPending = true;
            }
            return this;
        }

        public void update() {
            mPending = false;
            mText = toString();
            mCursorPosition = Selection.getSelectionStart(this);
            mAnchorPosition = Selection.getSelectionEnd(this);
            SlintAndroidJavaHelper.updateText(mText, mCursorPosition, mAnchorPosition,
                    BaseInputConnection.getComposingSpanStart(this), BaseInputConnection.getComposingSpanEnd(this));
        }
    }

    public SlintInputView(Context context) {
        super(context);
        setFocusable(true);
        setFocusableInTouchMode(true);
        mEditable = new SlintEditable();
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        outAttrs.inputType = mInputType;
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI;
        outAttrs.initialSelStart = mCursorPosition;
        outAttrs.initialSelEnd = mAnchorPosition;
        return new BaseInputConnection(this, true) {
            @Override
            public Editable getEditable() {
                return mEditable;
            }

            @Override
            public boolean beginBatchEdit() {
                mInBatch += 1;
                return super.beginBatchEdit();
            }

            @Override
            public boolean endBatchEdit() {
                mInBatch -= 1;
                if (mInBatch == 0 && mPending) {
                    mEditable.update();
                }
                return super.endBatchEdit();
            }
        };
    }

    public void setText(String text, int cursorPosition, int anchorPosition, int preeditStart, int preeditEnd,
            int inputType) {
        boolean typeChanged = mInputType != inputType;
        boolean textChanged = !mText.equals(text);
        boolean selectionChanged = mCursorPosition != cursorPosition || mAnchorPosition != anchorPosition;

        mText = text;
        mCursorPosition = cursorPosition;
        mAnchorPosition = anchorPosition;
        mInputType = inputType;

        if (typeChanged) {
            mEditable = new SlintEditable();
            Selection.setSelection(mEditable, cursorPosition, anchorPosition);
            getContext().getSystemService(InputMethodManager.class).restartInput(this);
        } else if (textChanged || selectionChanged) {
            mInBatch += 1;
            try {
                if (textChanged) {
                    mEditable.replace(0, mEditable.length(), text);
                }
                if (Selection.getSelectionStart(mEditable) != cursorPosition
                        || Selection.getSelectionEnd(mEditable) != anchorPosition) {
                    Selection.setSelection(mEditable, cursorPosition, anchorPosition);
                }
            } finally {
                mInBatch -= 1;
                mPending = false;
            }
            getContext().getSystemService(InputMethodManager.class)
                    .updateSelection(this, cursorPosition, anchorPosition, preeditStart, preeditEnd);
        }
    }

    @Override
    protected void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        SlintAndroidJavaHelper.setNightMode(newConfig.uiMode & Configuration.UI_MODE_NIGHT_MASK);
        SlintAndroidJavaHelper.setFontScale(newConfig.fontScale);
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

    private static void hideHandle(InputHandle handle) {
        if (handle != null) {
            handle.hide();
        }
    }

    public void hideCursor() {
        setCursorPos(0, 0, 0, 0, 0, 0);
    }

    // numHandles: 0=hidden, 1=cursor handle, 2=selection handles
    public void setCursorPos(int leftX, int leftY, int rightX, int rightY, int cursorHeight, int numHandles) {
        int handleHeight = 0;
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
            showActionMenu();
        } else {
            if (mCursorHandle != null) {
                handleHeight = mCursorHandle.getHeight();
            }
            hideHandle(mCursorHandle);
            hideHandle(mLeftHandle);
            hideHandle(mRightHandle);
            finishActionMenu();
        }

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
        if (mCurrentActionMode != null) {
            mCurrentActionMode.hide(0);
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

                // The ids are the ones callback_popup_menu_action in javahelper.rs expects.
                menu.add(Menu.FIRST, 0, 0, android.R.string.cut)
                        .setAlphabeticShortcut('x')
                        .setIcon(a.getDrawable(0));
                menu.add(Menu.FIRST, 1, 1, android.R.string.copy)
                        .setAlphabeticShortcut('c')
                        .setIcon(a.getDrawable(1));
                menu.add(Menu.FIRST, 2, 2, android.R.string.paste)
                        .setAlphabeticShortcut('v')
                        .setIcon(a.getDrawable(2));
                menu.add(Menu.FIRST, 3, 3, android.R.string.selectAll)
                        .setAlphabeticShortcut('a')
                        .setIcon(a.getDrawable(3));

                a.recycle();

                return true;
            }

            @Override
            public boolean onPrepareActionMode(ActionMode mode, Menu menu) {
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
                outRect.set(mSelectionRect);
                if (outRect.top < 0) {
                    // FIXME: I don't know why this is the case, but without that, the menu doesn't
                    // show at the right position when there is no room on top.
                    // Looks like the menu is always shown at outRect.top.
                    outRect.top = outRect.bottom;
                }
            }
        };
        mCurrentActionMode = startActionMode(action, ActionMode.TYPE_FLOATING);
    }

    void finishActionMenu() {
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

    public static native void popupMenuAction(int id);

    public static native void setInsets(int windowTop, int windowLeft, int windowBottom, int windowRight,
            int safeAreaTop, int safeAreaLeft, int safeAreaBottom, int safeAreaRight,
            int keyboardTop, int keyboardLeft, int keyboardBottom, int keyboardRight);

    public void set_imm_data(String text, int cursorPosition, int anchorPosition, int preeditStart, int preeditEnd,
            int curX, int curY, int anchorX, int anchorY, int cursorHeight, int inputType,
            boolean showCursorHandles) {
        mActivity.runOnUiThread(() -> {
            int selStart = Math.min(cursorPosition, anchorPosition);
            int selEnd = Math.max(cursorPosition, anchorPosition);
            mInputView.setText(text, selStart, selEnd, preeditStart, preeditEnd, inputType);
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
