/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include "main.h"

#include <Spix/SlintBot.h>

#include <BasicTests.h>

int main()
{
    auto ui = MainWindow::create();
    ui->show();

    BasicTests tests(/* quitWhenDone */ true);
    spix::SlintBot bot;
    bot.addWindow("mainWindow", ui);
    bot.runTestServer(tests);

    slint::run_event_loop();
    return tests.passed() ? 0 : 1;
}
