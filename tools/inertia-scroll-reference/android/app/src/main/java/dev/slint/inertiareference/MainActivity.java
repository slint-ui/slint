// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package dev.slint.inertiareference;

import android.app.Activity;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.os.Bundle;
import android.util.Log;
import android.view.Choreographer;
import android.view.View;
import android.widget.OverScroller;

public final class MainActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(new ProbeView(this));
    }

    private static final class ProbeView extends View implements Choreographer.FrameCallback {
        private static final String TAG = "inertia-reference";
        private static final String GESTURE = "medium-flick";
        private static final int CONTENT_HEIGHT = 4000;
        private static final int VIEWPORT_HEIGHT = 480;
        private static final int RELEASE_Y = 120;
        private static final int RELEASE_VELOCITY_Y = 2500;

        private final OverScroller scroller;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private int frame;
        private long firstFrameNanos;
        private int previousY = RELEASE_Y;
        private String phase = "released";

        ProbeView(Context context) {
            super(context);
            scroller = new OverScroller(context);
            setBackgroundColor(Color.WHITE);
        }

        @Override
        protected void onAttachedToWindow() {
            super.onAttachedToWindow();
            Log.i(TAG, "source,gesture,frame,time_ms,y_px,velocity_px_s,phase");
            scroller.fling(
                    0,
                    RELEASE_Y,
                    0,
                    RELEASE_VELOCITY_Y,
                    0,
                    0,
                    0,
                    CONTENT_HEIGHT - VIEWPORT_HEIGHT);
            Choreographer.getInstance().postFrameCallback(this);
        }

        @Override
        public void doFrame(long frameTimeNanos) {
            if (firstFrameNanos == 0) {
                firstFrameNanos = frameTimeNanos;
            }

            boolean moving = scroller.computeScrollOffset();
            int y = scroller.getCurrY();
            int elapsedMs = (int) ((frameTimeNanos - firstFrameNanos) / 1_000_000L);
            float velocity = frame == 0 ? 0.0f : (y - previousY) * 1000.0f / 16.0f;
            phase = moving ? "inertia" : "stopped";
            previousY = y;

            Log.i(
                    TAG,
                    "android-over-scroller,"
                            + GESTURE
                            + ","
                            + frame
                            + ","
                            + elapsedMs
                            + ","
                            + y
                            + ","
                            + velocity
                            + ","
                            + phase);

            frame += 1;
            invalidate();

            if (moving) {
                Choreographer.getInstance().postFrameCallback(this);
            }
        }

        @Override
        protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            int y = scroller.getCurrY();
            canvas.save();
            canvas.translate(0, -y);
            for (int row = 0; row < 80; row++) {
                paint.setColor(row % 2 == 0 ? Color.rgb(246, 248, 251) : Color.rgb(216, 227, 236));
                canvas.drawRect(0, row * 50, getWidth(), row * 50 + 50, paint);
                paint.setColor(row % 5 == 0 ? Color.rgb(245, 158, 11) : Color.rgb(59, 130, 246));
                canvas.drawRect(0, row * 50, 4, row * 50 + 50, paint);
            }
            canvas.restore();

            paint.setColor(Color.argb(220, 17, 24, 39));
            canvas.drawRect(0, 0, getWidth(), 112, paint);
            paint.setTextSize(18);
            paint.setColor(Color.WHITE);
            canvas.drawText(GESTURE, 12, 30, paint);
            paint.setTextSize(15);
            paint.setColor(Color.rgb(209, 213, 219));
            canvas.drawText("phase=" + phase + " frame=" + frame, 12, 56, paint);
            canvas.drawText("y=" + y + "px", 12, 80, paint);
        }
    }
}
