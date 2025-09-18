// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import java.util.concurrent.Callable;
import java.util.concurrent.FutureTask;
import android.view.ActionMode;
import android.view.Menu;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.Configuration;
import android.content.res.TypedArray;
import android.graphics.BlendMode;
import android.graphics.BlendModeColorFilter;
import android.graphics.PorterDuff;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;
import android.text.Editable;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.util.TypedValue;
import android.view.inputmethod.InputMethodManager;
import android.app.Activity;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.PopupWindow;
import android.view.inputmethod.BaseInputConnection;

class InputHandle extends ImageView {
    private PopupWindow mPopupWindow;
    private float mPressedX;
    private float mPressedY;
    private SlintInputView mRootView;
    private int cursorX;
    private int cursorY;
    private int attr;

    public InputHandle(SlintInputView rootView, int attr) {
        super(rootView.getContext());
        this.attr = attr;
        mRootView = rootView;
        Context ctx = rootView.getContext();
        mPopupWindow = new PopupWindow(ctx, null, android.R.attr.textSelectHandleWindowStyle);
        mPopupWindow.setSplitTouchEnabled(true);
        mPopupWindow.setClippingEnabled(false);
        int[] attrs = { attr };
        Drawable drawable = ctx.getTheme().obtainStyledAttributes(attrs).getDrawable(0);
        mPopupWindow.setWidth(drawable.getIntrinsicWidth());
        mPopupWindow.setHeight(drawable.getIntrinsicHeight());
        this.setImageDrawable(drawable);
        mPopupWindow.setContentView(this);
    }

    @Override
    public boolean onTouchEvent(MotionEvent ev) {
        switch (ev.getActionMasked()) {
            case MotionEvent.ACTION_DOWN: {
                mPressedX = ev.getRawX() - cursorX;
                mPressedY = ev.getRawY() - cursorY;
                break;
            }

            case MotionEvent.ACTION_MOVE: {
                mRootView.hideActionMenu(ActionMode.DEFAULT_HIDE_DURATION);
                int id = attr == android.R.attr.textSelectHandleLeft ? 1
                        : attr == android.R.attr.textSelectHandleRight ? 2 : 0;
                SlintAndroidJavaHelper.moveCursorHandle(id, Math.round(ev.getRawX() - mPressedX),
                        Math.round(ev.getRawY() - mPressedY));
                break;
            }
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_CANCEL:
                break;
        }
        return true;
    }

    public void setPosition(int x, int y) {
        cursorX = x;
        cursorY = y;

        y += mPopupWindow.getHeight();
        if (attr == android.R.attr.textSelectHandleLeft) {
            x -= 3 * mPopupWindow.getWidth() / 4;
        } else if (attr == android.R.attr.textSelectHandleRight) {
            x -= mPopupWindow.getWidth() / 4;
        } else {
            x -= mPopupWindow.getWidth() / 2;
        }

        mPopupWindow.showAtLocation(mRootView, 0, x, y);
        mPopupWindow.update(x, y, -1, -1);
    }

    public void hide() {
        mPopupWindow.dismiss();
    }

    public void setHandleColor(int color) {
        Drawable drawable = getDrawable();
        if (drawable != null) {
            if (android.os.Build.VERSION.SDK_INT >= 29) {
                drawable.setColorFilter(new BlendModeColorFilter(color, BlendMode.SRC_IN));
            } else {
                drawable.setColorFilter(color, PorterDuff.Mode.SRC_IN);
            }
            setImageDrawable(drawable);
        }
    }
}

class SlintInputView extends View {
    private String mText = "";
    private int mCursorPosition = 0;
    private int mAnchorPosition = 0;
    private int mPreeditStart = 0;
    private int mPreeditEnd = 0;
    private int mInputType = EditorInfo.TYPE_CLASS_TEXT;
    private int mInBatch = 0;
    private boolean mPending = false;
    private SlintEditable mEditable;

    public class SlintEditable extends SpannableStringBuilder {
        public SlintEditable() {
            super(mText);
        }

        @Override
        public SpannableStringBuilder replace(int start, int end, CharSequence tb, int tbstart, int tbend) {
            super.replace(start, end, tb, tbstart, tbend);
            setCursorPos(0, 0, 0, 0, 0, 0);
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
            mPreeditStart = BaseInputConnection.getComposingSpanStart(this);
            mPreeditEnd = BaseInputConnection.getComposingSpanEnd(this);
            SlintAndroidJavaHelper.updateText(mText, mCursorPosition, mAnchorPosition, mPreeditStart, mPreeditEnd);
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
        boolean restart = mInputType != inputType || !mText.equals(text) || mCursorPosition != cursorPosition
                || mAnchorPosition != anchorPosition;
        mText = text;
        mCursorPosition = cursorPosition;
        mAnchorPosition = anchorPosition;
        mPreeditStart = preeditStart;
        mPreeditEnd = preeditEnd;
        mInputType = inputType;

        if (restart) {
            mEditable = new SlintEditable();
            Selection.setSelection(mEditable, cursorPosition, anchorPosition);
            InputMethodManager imm = (InputMethodManager) this.getContext()
                    .getSystemService(Context.INPUT_METHOD_SERVICE);
            imm.restartInput(this);
        }
    }

    @Override
    protected void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        int currentNightMode = newConfig.uiMode & Configuration.UI_MODE_NIGHT_MASK;
        SlintAndroidJavaHelper.setNightMode(currentNightMode);
    }

    private InputHandle mCursorHandle;
    private InputHandle mLeftHandle;
    private InputHandle mRightHandle;
    public Rect selectionRect = new Rect();

    // num_handles: 0=hidden, 1=cursor handle, 2=selection handles
    public void setCursorPos(int left_x, int left_y, int right_x, int right_y, int cursor_height, int num_handles) {
        int handleHeight = 0;
        if (num_handles == 1) {
            if (mLeftHandle != null) {
                mLeftHandle.hide();
            }
            if (mRightHandle != null) {
                mRightHandle.hide();
            }
            if (mCursorHandle == null) {
                mCursorHandle = new InputHandle(this, android.R.attr.textSelectHandle);
            }
            mCursorHandle.setPosition(left_x, left_y);
            handleHeight = mCursorHandle.getHeight();
        } else if (num_handles == 2) {
            if (left_x != -1) {
                if (mLeftHandle == null) {
                    mLeftHandle = new InputHandle(this, android.R.attr.textSelectHandleLeft);
                }
                mLeftHandle.setPosition(left_x, left_y);
                handleHeight = mLeftHandle.getHeight();
            } else {
                if (mLeftHandle != null) {
                    mLeftHandle.hide();
                }
            }
            if (right_x != -1) {
                if (mRightHandle == null) {
                    mRightHandle = new InputHandle(this, android.R.attr.textSelectHandleRight);
                }
                mRightHandle.setPosition(right_x, right_y);
                handleHeight = mRightHandle.getHeight();
            } else {
                if (mRightHandle != null) {
                    mRightHandle.hide();
                }
            }
            if (mCursorHandle != null) {
                mCursorHandle.hide();
            }
            showActionMenu();
        } else {
            if (mCursorHandle != null) {
                handleHeight = mCursorHandle.getHeight();
                mCursorHandle.hide();
            }
            if (mLeftHandle != null) {
                mLeftHandle.hide();
            }
            if (mRightHandle != null) {
                mRightHandle.hide();
            }
            hideActionMenu(-1);
        }

        selectionRect.set(Math.min(left_x, right_x), Math.min(left_y, right_y) - cursor_height,
                Math.max(left_x, right_x), Math.max(left_y, right_y) + handleHeight);
        if (mCurrentActionMode != null) {
            mCurrentActionMode.invalidateContentRect();
        }
    }

    public void setHandleColor(int color) {
        if (mCursorHandle != null) {
            mCursorHandle.setHandleColor(color);
        }
        if (mLeftHandle != null) {
            mLeftHandle.setHandleColor(color);
        }
        if (mRightHandle != null) {
            mRightHandle.setHandleColor(color);
        }
    }

    private ActionMode mCurrentActionMode;

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
                if (android.os.Build.VERSION.SDK_INT >= 28) {
                    menu.setGroupDividerEnabled(true);
                }

                final TypedArray a = getContext().obtainStyledAttributes(new int[] {
                        android.R.attr.actionModeCutDrawable,
                        android.R.attr.actionModeCopyDrawable,
                        android.R.attr.actionModePasteDrawable,
                        android.R.attr.actionModeSelectAllDrawable,
                });

                // Note: the ids are used in Java_SlintAndroidJavaHelper_popupMenuAction
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
            }

            // Introduced in API level 23
            @Override
            public void onGetContentRect(ActionMode mode, View view, Rect outRect) {
                outRect.set(selectionRect);
                int actionBarHeight = 0;
                TypedValue tv = new TypedValue();
                if (getContext().getTheme().resolveAttribute(android.R.attr.actionBarSize, tv, true)) {
                    actionBarHeight = TypedValue.complexToDimensionPixelSize(tv.data,
                            getContext().getResources().getDisplayMetrics());
                }
                outRect.top -= actionBarHeight;
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

    public void hideActionMenu(int duration) {
        if (mCurrentActionMode != null) {
            if (duration < 0) {
                mCurrentActionMode.finish();
                mCurrentActionMode = null;
            } else {
                mCurrentActionMode.hide(duration);
            }
        }
    }
}

public class SlintAndroidJavaHelper {
    Activity mActivity;
    SlintInputView mInputView;

    public SlintAndroidJavaHelper(Activity activity) {
        this.mActivity = activity;
        this.mInputView = new SlintInputView(activity);
        this.mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT);
                mActivity.addContentView(mInputView, params);
                mInputView.setVisibility(View.VISIBLE);
            }
        });
    }

    public void show_keyboard() {
        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                mInputView.requestFocus();
                InputMethodManager imm = (InputMethodManager) mActivity.getSystemService(Context.INPUT_METHOD_SERVICE);
                imm.showSoftInput(mInputView, 0);
            }
        });
    }

    public void hide_keyboard() {
        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                InputMethodManager imm = (InputMethodManager) mActivity.getSystemService(Context.INPUT_METHOD_SERVICE);
                imm.hideSoftInputFromWindow(mInputView.getWindowToken(), 0);
                mInputView.clearFocus();
                mInputView.setCursorPos(0, 0, 0, 0, 0, 0);
            }
        });
    }

    static public native void updateText(String text, int cursorPosition, int anchorPosition, int preeditStart,
            int preeditOffset);

    static public native void setNightMode(int nightMode);

    static public native void moveCursorHandle(int id, int pos_x, int pos_y);

    static public native void popupMenuAction(int id);

    public void set_imm_data(String text, int cursor_position, int anchor_position, int preedit_start, int preedit_end,
            int cur_x, int cur_y, int anchor_x, int anchor_y, int cursor_height, int input_type,
            boolean show_cursor_handles) {

        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                int selStart = Math.min(cursor_position, anchor_position);
                int selEnd = Math.max(cursor_position, anchor_position);
                mInputView.setText(text, selStart, selEnd, preedit_start, preedit_end, input_type);
                int num_handles = 0;
                if (show_cursor_handles) {
                    num_handles = cursor_position == anchor_position ? 1 : 2;
                }
                if (cursor_position < anchor_position) {
                    mInputView.setCursorPos(cur_x, cur_y, anchor_x, anchor_y, cursor_height, num_handles);
                } else {
                    mInputView.setCursorPos(anchor_x, anchor_y, cur_x, cur_y, cursor_height, num_handles);
                }

            }
        });
    }

    public void set_handle_color(int color) {
        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                mInputView.setHandleColor(color);
            }
        });
    }

    public int color_scheme() {
        int nightModeFlags = mActivity.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK;
        return nightModeFlags;
    }

    // Get the geometry of the view minus the system bars and the keyboard
    public Rect get_view_rect() {
        Rect rect = new Rect();
        mActivity.getWindow().getDecorView().getWindowVisibleDisplayFrame(rect);
        // Note: `View.getRootWindowInsets` requires API level 23 or above
        WindowInsets insets = mActivity.getWindow().getDecorView().getRootView().getRootWindowInsets();
        if (insets != null) {
            int dx = rect.left - insets.getSystemWindowInsetLeft();
            int dy = rect.top - insets.getSystemWindowInsetTop();

            rect.left -= dx;
            rect.right -= dx;
            rect.top -= dy;
            rect.bottom -= dy;
        }
        return rect;
    }

    public void show_action_menu() {
        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                mInputView.showActionMenu();
            }
        });
    }

    public String get_clipboard() {
        FutureTask<String> future = new FutureTask<>(new Callable<String>() {
            @Override
            public String call() throws Exception {
                ClipboardManager clipboard = (ClipboardManager) mActivity.getSystemService(Context.CLIPBOARD_SERVICE);
                if (clipboard.hasPrimaryClip()) {
                    ClipData.Item item = clipboard.getPrimaryClip().getItemAt(0);
                    return item.getText().toString();
                }
                return "";
            }
        });

        mActivity.runOnUiThread(future);
        try {
            return future.get(); // Wait for the result and return it
        } catch (Exception e) {
            e.printStackTrace();
            return "";
        }
    }

    public void set_clipboard(String text) {
        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                ClipboardManager clipboard = (ClipboardManager) mActivity.getSystemService(Context.CLIPBOARD_SERVICE);
                ClipData clip = ClipData.newPlainText(null, text);
                clipboard.setPrimaryClip(clip);
            }
        });
    }
}
