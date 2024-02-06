// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Rect;
import android.view.inputmethod.InputMethodManager;
import android.app.Activity;
import android.widget.FrameLayout;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.ExtractedText;

class SlintInputView extends View {
    private String mText = "";
    private int mCursorPosition = 0;
    private int mAnchorPosition = 0;
    private String mPreedit = "";
    private int mPreeditOffset;
    private int mInputType = EditorInfo.TYPE_CLASS_TEXT;

    public SlintInputView(Context context) {
        super(context);
        setFocusable(true);
        setFocusableInTouchMode(true);
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
        return new BaseInputConnection(this, false) {
            @Override
            public CharSequence getTextBeforeCursor(int n, int flags) {
                return mText.substring(0, mCursorPosition);
            }

            @Override
            public CharSequence getTextAfterCursor(int n, int flags) {
                return mText.substring(mCursorPosition);
            }

            @Override
            public CharSequence getSelectedText(int flags) {
                if (mCursorPosition != mAnchorPosition) {
                    return mText.substring(mCursorPosition, mAnchorPosition);
                }
                return null;
            }

            @Override
            public boolean commitText(CharSequence text, int newCursorPosition) {
                mText = new StringBuilder(mText).delete(mCursorPosition, mAnchorPosition).insert(mCursorPosition, text)
                        .toString();
                mPreedit = "";
                if (newCursorPosition > 0) {
                    mCursorPosition = mCursorPosition + text.length() + newCursorPosition - 1;
                } else {
                    mCursorPosition = mCursorPosition + newCursorPosition;
                }
                mAnchorPosition = mCursorPosition;
                SlintAndroidJavaHelper.updateText(mText, mCursorPosition, mAnchorPosition, mPreedit, mPreeditOffset);
                // return super.commitText(text, newCursorPosition);
                return true;
            }

            @Override
            public boolean deleteSurroundingText(int beforeLength, int afterLength) {
                mText = new StringBuilder(mText).delete(mCursorPosition - beforeLength, mAnchorPosition + afterLength)
                        .toString();
                mCursorPosition = mCursorPosition - beforeLength;
                mAnchorPosition = mCursorPosition;
                SlintAndroidJavaHelper.updateText(mText, mCursorPosition, mAnchorPosition, mPreedit, mPreeditOffset);
                return true;
            }

            @Override
            public boolean setComposingText(CharSequence text, int newCursorPosition) {
                mPreedit = text.toString();
                mPreeditOffset = newCursorPosition;
                SlintAndroidJavaHelper.updateText(mText, mCursorPosition, mAnchorPosition, mPreedit, mPreeditOffset);
                return super.setComposingText(text, newCursorPosition);
            }

            @Override
            public boolean setSelection(int start, int end) {
                mCursorPosition = start;
                mAnchorPosition = end;
                SlintAndroidJavaHelper.updateText(mText, mCursorPosition, mAnchorPosition, mPreedit, mPreeditOffset);
                return true;
            }
        };
    }

    public void setText(String text, int cursorPosition, int anchorPosition, String preedit, int preeditOffset,
            int inputType) {
        boolean restart = mInputType != inputType || !mText.equals(text);
        boolean update_selection = mCursorPosition != cursorPosition || mAnchorPosition != anchorPosition;
        mText = text;
        mCursorPosition = cursorPosition;
        mAnchorPosition = anchorPosition;
        mPreedit = preedit;
        mPreeditOffset = preeditOffset;
        mInputType = inputType;

        InputMethodManager imm = (InputMethodManager) this.getContext().getSystemService(Context.INPUT_METHOD_SERVICE);
        if (restart) {
            imm.restartInput(this);
        } else if (update_selection) {
            ExtractedText extractedText = new ExtractedText();
            extractedText.text = mText;
            extractedText.startOffset = mPreeditOffset;
            extractedText.selectionStart = mCursorPosition;
            extractedText.selectionEnd = mAnchorPosition;
            imm.updateExtractedText(this, 0, extractedText);
            imm.updateSelection(this, cursorPosition, anchorPosition, cursorPosition, anchorPosition);
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

    static public native void updateText(String text, int cursorPosition, int anchorPosition, String preedit,
            int preeditOffset);

    static public native void setDarkMode(boolean dark);

    public void set_imm_data(String text, int cursor_position, int anchor_position, String preedit, int preedit_offset,
            int rect_x, int rect_y, int rect_w, int rect_h, int input_type) {

        mActivity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                FrameLayout.LayoutParams layoutParams = new FrameLayout.LayoutParams(rect_w, rect_h);
                layoutParams.setMargins(rect_x, rect_y, 0, 0);
                mInputView.setLayoutParams(layoutParams);
                int selStart = Math.min(cursor_position, anchor_position);
                int selEnd = Math.max(cursor_position, anchor_position);
                mInputView.setText(text, selStart, selEnd, preedit, preedit_offset, input_type);
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
