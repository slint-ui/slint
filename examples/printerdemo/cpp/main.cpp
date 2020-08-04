#include "printerdemo.h"
#include <iostream>

int main()
{
    static MainWindow printer_demo;

    sixtyfps::ComponentWindow window;
    window.run(&printer_demo);
}
