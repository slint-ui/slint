// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import android.view.View;
import android.content.Context;
import android.view.inputmethod.InputMethodManager;
import android.app.Activity;

public class SlintAndroidJavaHelper  {
    Activity mActivity;

    public SlintAndroidJavaHelper(Activity activity) {
        this.mActivity = activity;
    }
    public void show_keyboard() {
        InputMethodManager imm = (InputMethodManager)mActivity.getSystemService(Context.INPUT_METHOD_SERVICE);
        imm.showSoftInput(mActivity.getWindow().getDecorView(), 0);
    }
    public void hide_keyboard() {
        InputMethodManager imm = (InputMethodManager)mActivity.getSystemService(Context.INPUT_METHOD_SERVICE);
        imm.hideSoftInputFromWindow(mActivity.getWindow().getDecorView().getWindowToken(), 0);
    }

}
