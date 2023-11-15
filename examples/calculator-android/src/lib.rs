#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    use std::cell::RefCell;
    use std::rc::Rc;

    slint::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new(app),
    ))
    .unwrap();

    let app = MainWindow::new().unwrap();
    let weak = app.as_weak();

    // press first number
    let mut num1 = 0;

    // press second number
    let mut num2 = 0;

    // operator
    let operator = Rc::new(RefCell::new(String::new()));

    app.global::<Calculator>().on_click(move |text| {
        let app = weak.unwrap();
        if text == "AC" {
            app.set_result(0);
            num1 = 0;
            num2 = 0;
            operator.borrow_mut().clear();
            return;
        }

        if text == "+" || text == "-" || text == "x" || text == "÷" {
            num1 = app.get_result();
            operator.borrow_mut().clear();
            operator.borrow_mut().push_str(text.as_str());
            app.set_result(0);
            return;
        }

        if text == "=" {
            num2 = app.get_result();
            match operator.borrow().as_str() {
                "+" => {
                    let (result, overflowed) = num1.overflowing_add(num2);
                    if overflowed {
                        app.set_result(0);
                    } else {
                        app.set_result(result);
                    }
                }
                "-" => {
                    let (result, overflowed) = num1.overflowing_sub(num2);
                    if overflowed {
                        app.set_result(0);
                    } else {
                        app.set_result(result);
                    }
                }
                "x" => {
                    let (result, overflowed) = num1.overflowing_mul(num2);
                    if overflowed {
                        app.set_result(0);
                    } else {
                        app.set_result(result);
                    }
                }
                "÷" => {
                    if num2 == 0 {
                        app.set_result(0);
                    } else {
                        app.set_result(num1 / num2);
                    }
                }
                _ => {}
            }
            operator.borrow_mut().clear();
            operator.borrow_mut().push_str("=");
            return;
        }

        if let Ok(value) = text.parse::<i32>() {
            let current;
            if operator.borrow().as_str() == "=" {
                app.set_result(0);
                num1 = 0;
                current = 0;
                operator.borrow_mut().clear();
            } else {
                current = app.get_result();
            }
            let (result, overflowed) = current.overflowing_mul(10);
            if overflowed {
                app.set_result(0);
                return;
            }
            let (result, overflowed) = result.overflowing_add(value);
            if overflowed {
                app.set_result(0);
                return;
            }
            app.set_result(result);
            return;
        }
    });
    app.run().unwrap();
}

// UI
slint::slint! {
    import { GridBox , Button} from "std-widgets.slint";

    export global Calculator {
        // click event
        callback click(string);
    }

    export component MainWindow inherits Window {
        // Result
        in-out property <int> result: 0;

        GridLayout {
            padding-top: 100px;
            padding-left: 20px;
            padding-right: 20px;
            spacing: 10px;

            Row {
                Text {
                    colspan: 3;
                    text: result;
                    height: 40px;
                    font-size: 20px;
                }
            }

            Row {
                Button {
                    colspan: 3;
                    height: 80px;
                    text: "AC";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    col: 3;
                    colspan: 1;
                    height: 80px;
                    text: "÷";
                    clicked => { Calculator.click(self.text)}
                }
            }

            Row {
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "7";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "8";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "9";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    rowspan: 2;
                    height: 80px;
                    text: "x";
                    clicked => { Calculator.click(self.text)}
                }
            }

            Row {
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "4";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "5";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "6";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "-";
                    clicked => { Calculator.click(self.text)}
                }
            }

            Row {
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "1";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "2";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "3";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "+";
                    clicked => { Calculator.click(self.text)}
                }
            }

            Row {
                Button {
                    colspan: 1;
                    height: 80px;
                    text: "0";
                    clicked => { Calculator.click(self.text)}
                }
                Button {
                    colspan: 3;
                    height: 80px;
                    text: "=";
                    clicked => { Calculator.click(self.text)}
                }
            }
        }
    }
}
