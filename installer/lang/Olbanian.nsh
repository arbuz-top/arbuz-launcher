;Language: Olbanian
;Translation updated by Dmitry Yerokhin [erodim@mail.ru] (050424)

!insertmacro LANGFILE "Olbanian" "Olbanian" "Olbanian" "Olbanian"

!ifdef MUI_WELCOMEPAGE
  ${LangFileString} MUI_TEXT_WELCOME_INFO_TITLE "Васъ приветствуетъ мастеръ установки $(^NameDA)"
  ${LangFileString} MUI_TEXT_WELCOME_INFO_TEXT "Эта программа установитъ $(^NameDA) на вашъ компьютеръ.$\r$\n$\r$\nПередъ началомъ установки рекомендуетсйа закрыть все работающие приложенийа. Это позволитъ программе установки обновить системные файлы безъ перезагрузки компьютера.$\r$\n$\r$\n$_CLICK"
!endif

!ifdef MUI_UNWELCOMEPAGE
  ${LangFileString} MUI_UNTEXT_WELCOME_INFO_TITLE "Васъ приветствуетъ мастеръ удаленийа $(^NameDA)"
  ${LangFileString} MUI_UNTEXT_WELCOME_INFO_TEXT "Эта программа удалитъ $(^NameDA) из вашего компьютера.$\r$\n$\r$\nПередъ началомъ удаленийа убедитесь, что программа $(^NameDA) не запущена.$\r$\n$\r$\n$_CLICK"
!endif

!ifdef MUI_LICENSEPAGE
  ${LangFileString} MUI_TEXT_LICENSE_TITLE "Лицензионное соглашение"
  ${LangFileString} MUI_TEXT_LICENSE_SUBTITLE "Передъ установкой $(^NameDA) ознакомьтесь с лицензионнымъ соглашениемъ."
  ${LangFileString} MUI_INNERTEXT_LICENSE_BOTTOM "Если вы принимаете условийа соглашенийа, нажмите кнопку $\"Принимаю$\". Чтобы установить программу, необходимо принйать соглашение."
  ${LangFileString} MUI_INNERTEXT_LICENSE_BOTTOM_CHECKBOX "Если вы принимаете условийа соглашенийа, установите флажокъ ниже. Чтобы установить программу, необходимо принйать соглашение. $_CLICK"
  ${LangFileString} MUI_INNERTEXT_LICENSE_BOTTOM_RADIOBUTTONS "Если вы принимаете условийа соглашенийа, выберите первый вариантъ из предложенныхъ ниже. Чтобы установить программу, необходимо принйать соглашение. $_CLICK"
!endif

!ifdef MUI_UNLICENSEPAGE
  ${LangFileString} MUI_UNTEXT_LICENSE_TITLE "Лицензионное соглашение"
  ${LangFileString} MUI_UNTEXT_LICENSE_SUBTITLE "Передъ удалениемъ $(^NameDA) ознакомьтесь с лицензионнымъ соглашениемъ."
  ${LangFileString} MUI_UNINNERTEXT_LICENSE_BOTTOM "Если вы принимаете условийа соглашенийа, нажмите кнопку $\"Принимаю$\". Длйа удаленийа необходимо принйать соглашение. $_CLICK"
  ${LangFileString} MUI_UNINNERTEXT_LICENSE_BOTTOM_CHECKBOX "Если вы принимаете условийа соглашенийа, установите флажокъ ниже. Длйа удаленийа необходимо принйать соглашение. $_CLICK"
  ${LangFileString} MUI_UNINNERTEXT_LICENSE_BOTTOM_RADIOBUTTONS "Если вы принимаете условийа соглашенийа, выберите первый вариантъ из предложенныхъ ниже. Длйа удаленийа необходимо принйать соглашение. $_CLICK"
!endif

!ifdef MUI_LICENSEPAGE | MUI_UNLICENSEPAGE
  ${LangFileString} MUI_INNERTEXT_LICENSE_TOP "Длйа перемещенийа по тексту используйте клавишы $\"PageUp$\" и $\"PageDown$\"."
!endif

!ifdef MUI_COMPONENTSPAGE
  ${LangFileString} MUI_TEXT_COMPONENTS_TITLE "Компоненты устанавливаемой программы"
  ${LangFileString} MUI_TEXT_COMPONENTS_SUBTITLE "Выберите компоненты $(^NameDA), которые вы хотите установить."
!endif

!ifdef MUI_UNCOMPONENTSPAGE
  ${LangFileString} MUI_UNTEXT_COMPONENTS_TITLE "Компоненты программы"
  ${LangFileString} MUI_UNTEXT_COMPONENTS_SUBTITLE "Выберите компоненты $(^NameDA), которые вы хотите удалить."
!endif

!ifdef MUI_COMPONENTSPAGE | MUI_UNCOMPONENTSPAGE
  ${LangFileString} MUI_INNERTEXT_COMPONENTS_DESCRIPTION_TITLE "Описание"
  !ifndef NSIS_CONFIG_COMPONENTPAGE_ALTERNATIVE
    ${LangFileString} MUI_INNERTEXT_COMPONENTS_DESCRIPTION_INFO "Наведите курсоръ мышы на название компонента, чтобы прочесть его описание."
  !else
    ${LangFileString} MUI_INNERTEXT_COMPONENTS_DESCRIPTION_INFO "Наведите курсоръ мышы на название компонента, чтобы прочесть его описание."
  !endif
!endif

!ifdef MUI_DIRECTORYPAGE
  ${LangFileString} MUI_TEXT_DIRECTORY_TITLE "Выборъ папки установки"
  ${LangFileString} MUI_TEXT_DIRECTORY_SUBTITLE "Выберите папку длйа установки $(^NameDA)."
!endif

!ifdef MUI_UNDIRECTORYPAGE
  ${LangFileString} MUI_UNTEXT_DIRECTORY_TITLE "Выборъ папки длйа удаленийа"
  ${LangFileString} MUI_UNTEXT_DIRECTORY_SUBTITLE "Укажыте папку, из котарай нужно удалить $(^NameDA)."
!endif

!ifdef MUI_INSTFILESPAGE
  ${LangFileString} MUI_TEXT_INSTALLING_TITLE "Копирование файловъ"
  ${LangFileString} MUI_TEXT_INSTALLING_SUBTITLE "Подождите, идетъ копирование файловъ $(^NameDA)..."
  ${LangFileString} MUI_TEXT_FINISH_TITLE "Установка завершена"
  ${LangFileString} MUI_TEXT_FINISH_SUBTITLE "Установка успешно завершена."
  ${LangFileString} MUI_TEXT_ABORT_TITLE "Установка прервана"
  ${LangFileString} MUI_TEXT_ABORT_SUBTITLE "Установка не завершена."
!endif

!ifdef MUI_UNINSTFILESPAGE
  ${LangFileString} MUI_UNTEXT_UNINSTALLING_TITLE "Удаление"
  ${LangFileString} MUI_UNTEXT_UNINSTALLING_SUBTITLE "Подождите, идетъ удаление файловъ $(^NameDA)..."
  ${LangFileString} MUI_UNTEXT_FINISH_TITLE "Удаление завершено"
  ${LangFileString} MUI_UNTEXT_FINISH_SUBTITLE "Удаление программы успешно завершено."
  ${LangFileString} MUI_UNTEXT_ABORT_TITLE "Удаление прервано"
  ${LangFileString} MUI_UNTEXT_ABORT_SUBTITLE "Удаление произведено не полностью."
!endif

!ifdef MUI_FINISHPAGE
  ${LangFileString} MUI_TEXT_FINISH_INFO_TITLE "Завершение работы мастера установки $(^NameDA)"
  ${LangFileString} MUI_TEXT_FINISH_INFO_TEXT "Установка $(^NameDA) выполнена.$\r$\n$\r$\nНажмите кнопку $\"Готово$\" длйа выхода из программы установки."
  ${LangFileString} MUI_TEXT_FINISH_INFO_REBOOT "Длйа завершенийа установки $(^NameDA) необходимо перезагрузить компьютеръ. Хотите сделать это сейчасъ?"
!endif

!ifdef MUI_UNFINISHPAGE
  ${LangFileString} MUI_UNTEXT_FINISH_INFO_TITLE "Завершение работы мастера удаленийа $(^NameDA)"
  ${LangFileString} MUI_UNTEXT_FINISH_INFO_TEXT "Программа $(^NameDA) удалена из вашего компьютера.$\r$\n$\r$\nНажмите кнопку $\"Готово$\"длйа выхода из программы удаленийа."
  ${LangFileString} MUI_UNTEXT_FINISH_INFO_REBOOT "Длйа завершенийа удаленийа $(^NameDA) нужно перезагрузить компьютеръ. Хотите сделать это сейчасъ?"
!endif

!ifdef MUI_FINISHPAGE | MUI_UNFINISHPAGE
  ${LangFileString} MUI_TEXT_FINISH_REBOOTNOW "Да, перезагрузить ПК сейчасъ"
  ${LangFileString} MUI_TEXT_FINISH_REBOOTLATER "Нетъ, йа перезагружу ПК позже"
  ${LangFileString} MUI_TEXT_FINISH_RUN "&Запустить $(^NameDA)"
  ${LangFileString} MUI_TEXT_FINISH_SHOWREADME "&Показать файлъ ReadMe"
  ${LangFileString} MUI_BUTTONTEXT_FINISH "&Готово"
!endif

!ifdef MUI_STARTMENUPAGE
  ${LangFileString} MUI_TEXT_STARTMENU_TITLE "Папка в меню $\"Пускъ$\""
  ${LangFileString} MUI_TEXT_STARTMENU_SUBTITLE "Выберите папку в меню $\"Пускъ$\" длйа размещенийа йарлыковъ программы."
  ${LangFileString} MUI_INNERTEXT_STARTMENU_TOP "Выберите папку в меню $\"Пускъ$\", куда будутъ помещены йарлыки программы. Вы также можете ввести другое имйа папки."
  ${LangFileString} MUI_INNERTEXT_STARTMENU_CHECKBOX "Не создавать йарлыки"
!endif

!ifdef MUI_UNCONFIRMPAGE
  ${LangFileString} MUI_UNTEXT_CONFIRM_TITLE "Удаление $(^NameDA)"
  ${LangFileString} MUI_UNTEXT_CONFIRM_SUBTITLE "Удаление $(^NameDA) из компьютера."
!endif

!ifdef MUI_ABORTWARNING
  ${LangFileString} MUI_TEXT_ABORTWARNING "Вы действительно хотите отменить установку $(^Name)?"
!endif

!ifdef MUI_UNABORTWARNING
  ${LangFileString} MUI_UNTEXT_ABORTWARNING "Вы действительно хотите отменить удаление $(^Name)?"
!endif

!ifdef MULTIUSER_INSTALLMODEPAGE
  ${LangFileString} MULTIUSER_TEXT_INSTALLMODE_TITLE "Режымъ установки"
  ${LangFileString} MULTIUSER_TEXT_INSTALLMODE_SUBTITLE "Выберите, длйа какихъ пользователей вы хотите установить $(^NameDA)."
  ${LangFileString} MULTIUSER_INNERTEXT_INSTALLMODE_TOP "Выберите, будете ли вы устанавливать $(^NameDA) только длйа себйа или длйа всехъ пользователей данного компьютера. $(^ClickNext)"
  ${LangFileString} MULTIUSER_INNERTEXT_INSTALLMODE_ALLUSERS "Установить длйа всехъ пользователей"
  ${LangFileString} MULTIUSER_INNERTEXT_INSTALLMODE_CURRENTUSER "Установить только длйа текущего пользователйа"
!endif
