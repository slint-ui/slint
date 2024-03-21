// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import android.view.MotionEvent;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.BlendMode;
import android.graphics.BlendModeColorFilter;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;
import android.text.Editable;
import android.text.Selection;
import android.text.SpannableStringBuilder;
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
            drawable.setColorFilter(new BlendModeColorFilter(color, BlendMode.SRC_IN));
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
            setCursorPos(0, 0, 0, 0, 0);
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
        switch (currentNightMode) {
            case Configuration.UI_MODE_NIGHT_NO:
                SlintAndroidJavaHelper.setDarkMode(false);
                break;
            case Configuration.UI_MODE_NIGHT_YES:
                SlintAndroidJavaHelper.setDarkMode(true);
                break;
        }
    }

    private InputHandle mCursorHandle;
    private InputHandle mLeftHandle;
    private InputHandle mRightHandle;

    // num_handles: 0=hidden, 1=cursor handle, 2=selection handles
    public void setCursorPos(int left_x, int left_y, int right_x, int right_y, int num_handles) {
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
        } else if (num_handles == 2) {
            if (mLeftHandle == null) {
                mLeftHandle = new InputHandle(this, android.R.attr.textSelectHandleLeft);
            }
            if (mRightHandle == null) {
                mRightHandle = new InputHandle(this, android.R.attr.textSelectHandleRight);
            }
            if (mCursorHandle != null) {
                mCursorHandle.hide();
            }
            mLeftHandle.setPosition(right_x, right_y);
            mRightHandle.setPosition(left_x, left_y);
        } else {
            if (mCursorHandle != null) {
                mCursorHandle.hide();
            }
            if (mLeftHandle != null) {
                mLeftHandle.hide();
            }
            if (mRightHandle != null) {
                mRightHandle.hide();
            }
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
                FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(10, 10);
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
            }
        });
    }

    static public native void updateText(String text, int cursorPosition, int anchorPosition, int preeditStart,
            int preeditOffset);

    static public native void setDarkMode(boolean dark);

    static public native void moveCursorHandle(int id, int pos_x, int pos_y);

    public void set_imm_data(String text, int cursor_position, int anchor_position, int preedit_start, int preedit_end,
            int cur_x, int cur_y, int anchor_x, int anchor_y, int input_type, boolean show_cursor_handles) {

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
                    mInputView.setCursorPos(anchor_x, anchor_y, cur_x, cur_y, num_handles);
                } else {
                    mInputView.setCursorPos(cur_x, cur_y, anchor_x, anchor_y, num_handles);
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

    public boolean dark_color_scheme() {
        int nightModeFlags = mActivity.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK;
        return nightModeFlags == Configuration.UI_MODE_NIGHT_YES;
    }

    // Get the geometry of the view minus the system bars and the keyboard
    public Rect get_view_rect() {
        Rect rect = new Rect();
        mActivity.getWindow().getDecorView().getWindowVisibleDisplayFrame(rect);
        return rect;
    }
}
