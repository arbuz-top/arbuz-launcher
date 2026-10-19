;Language: Kazakh
;By Joost Verburg

!insertmacro LANGFILE "Kazakh" "Kazakh" "Kazakh" "Kazakh"

!ifdef MUI_WELCOMEPAGE
  ${LangFileString} MUI_TEXT_WELCOME_INFO_TITLE "$(^NameDA) Setup қолданбасына қош келдіңіз"
  ${LangFileString} MUI_TEXT_WELCOME_INFO_TEXT "Орнату сізге орнату бойынша нұсқау береді $(^NameDA).$\r$\n$\r$\nIt орнатуды бастамас бұрын барлық басқа қолданбаларды жабу ұсынылады. Бұл компьютерді қайта іске қоспай-ақ тиісті жүйелік файлдарды жаңартуға мүмкіндік береді.$\r$\n$\r$\n$_CLICK"
!endif

!ifdef MUI_UNWELCOMEPAGE
  ${LangFileString} MUI_UNTEXT_WELCOME_INFO_TITLE "$(^NameDA) Жоюға қош келдіңіз"
  ${LangFileString} MUI_UNTEXT_WELCOME_INFO_TEXT "Орнату $(^NameDA).$\r$\n$\r$\nBefore жоюды бастау арқылы сізге нұсқау береді, $(^NameDA) іске қосылмағанына көз жеткізіңіз.$\r$\n$\r$\n$_CLICK"
!endif

!ifdef MUI_LICENSEPAGE
  ${LangFileString} MUI_TEXT_LICENSE_TITLE "Лицензиялық келісім"
  ${LangFileString} MUI_TEXT_LICENSE_SUBTITLE "$(^NameDA). орнатпас бұрын лицензия шарттарын қарап шығыңыз"
  ${LangFileString} MUI_INNERTEXT_LICENSE_BOTTOM "Келісімнің шарттарын қабылдасаңыз, жалғастыру үшін Мен келісемін түймесін басыңыз. $(^NameDA). орнату келісімін қабылдауыңыз керек"
  ${LangFileString} MUI_INNERTEXT_LICENSE_BOTTOM_CHECKBOX "Егер келісім шарттарын қабылдасаңыз, төмендегі құсбелгіні басыңыз. $(^NameDA). $_CLICK орнату келісімін қабылдауыңыз керек"
  ${LangFileString} MUI_INNERTEXT_LICENSE_BOTTOM_RADIOBUTTONS "Егер келісім шарттарын қабылдасаңыз, төмендегі бірінші опцияны таңдаңыз. $(^NameDA). $_CLICK орнату келісімін қабылдауыңыз керек"
!endif

!ifdef MUI_UNLICENSEPAGE
  ${LangFileString} MUI_UNTEXT_LICENSE_TITLE "Лицензиялық келісім"
  ${LangFileString} MUI_UNTEXT_LICENSE_SUBTITLE "$(^NameDA). жоймас бұрын лицензия шарттарын қарап шығыңыз"
  ${LangFileString} MUI_UNINNERTEXT_LICENSE_BOTTOM "Келісімнің шарттарын қабылдасаңыз, жалғастыру үшін Мен келісемін түймесін басыңыз. $(^NameDA). жою туралы келісімді қабылдауыңыз керек"
  ${LangFileString} MUI_UNINNERTEXT_LICENSE_BOTTOM_CHECKBOX "Егер келісім шарттарын қабылдасаңыз, төмендегі құсбелгіні басыңыз. $(^NameDA). $_CLICK бағдарламасын жою келісімін қабылдауыңыз керек"
  ${LangFileString} MUI_UNINNERTEXT_LICENSE_BOTTOM_RADIOBUTTONS "Егер келісім шарттарын қабылдасаңыз, төмендегі бірінші опцияны таңдаңыз. $(^NameDA). $_CLICK бағдарламасын жою келісімін қабылдауыңыз керек"
!endif

!ifdef MUI_LICENSEPAGE | MUI_UNLICENSEPAGE
  ${LangFileString} MUI_INNERTEXT_LICENSE_TOP "Келісімнің қалған бөлігін көру үшін Page Down түймесін басыңыз."
!endif

!ifdef MUI_COMPONENTSPAGE
  ${LangFileString} MUI_TEXT_COMPONENTS_TITLE "Компоненттерді таңдаңыз"
  ${LangFileString} MUI_TEXT_COMPONENTS_SUBTITLE "$(^NameDA) қолданбасының қандай мүмкіндіктерін орнатқыңыз келетінін таңдаңыз."
!endif

!ifdef MUI_UNCOMPONENTSPAGE
  ${LangFileString} MUI_UNTEXT_COMPONENTS_TITLE "Компоненттерді таңдаңыз"
  ${LangFileString} MUI_UNTEXT_COMPONENTS_SUBTITLE "$(^NameDA) қолданбасының қай мүмкіндіктерін жойғыңыз келетінін таңдаңыз."
!endif

!ifdef MUI_COMPONENTSPAGE | MUI_UNCOMPONENTSPAGE
  ${LangFileString} MUI_INNERTEXT_COMPONENTS_DESCRIPTION_TITLE "Сипаттама"
  !ifndef NSIS_CONFIG_COMPONENTPAGE_ALTERNATIVE
    ${LangFileString} MUI_INNERTEXT_COMPONENTS_DESCRIPTION_INFO "Сипаттаманы көру үшін тінтуірді компоненттің үстіне қойыңыз."
  !else
    ${LangFileString} MUI_INNERTEXT_COMPONENTS_DESCRIPTION_INFO "Сипаттаманы көру үшін тінтуірді компоненттің үстіне қойыңыз."
  !endif
!endif

!ifdef MUI_DIRECTORYPAGE
  ${LangFileString} MUI_TEXT_DIRECTORY_TITLE "Орналасу орнын таңдаңыз"
  ${LangFileString} MUI_TEXT_DIRECTORY_SUBTITLE "$(^NameDA). орнатылатын қалтаны таңдаңыз"
!endif

!ifdef MUI_UNDIRECTORYPAGE
  ${LangFileString} MUI_UNTEXT_DIRECTORY_TITLE "Орынды жою опциясын таңдаңыз"
  ${LangFileString} MUI_UNTEXT_DIRECTORY_SUBTITLE "Жойылатын қалтаны таңдаңыз $(^NameDA)."
!endif

!ifdef MUI_INSTFILESPAGE
  ${LangFileString} MUI_TEXT_INSTALLING_TITLE "Орнату"
  ${LangFileString} MUI_TEXT_INSTALLING_SUBTITLE "$(^NameDA) орнату кезінде күтіңіз."
  ${LangFileString} MUI_TEXT_FINISH_TITLE "Орнату аяқталды"
  ${LangFileString} MUI_TEXT_FINISH_SUBTITLE "Орнату сәтті аяқталды."
  ${LangFileString} MUI_TEXT_ABORT_TITLE "Орнату тоқтатылды"
  ${LangFileString} MUI_TEXT_ABORT_SUBTITLE "Орнату сәтті аяқталмады."
!endif

!ifdef MUI_UNINSTFILESPAGE
  ${LangFileString} MUI_UNTEXT_UNINSTALLING_TITLE "Жоюда"
  ${LangFileString} MUI_UNTEXT_UNINSTALLING_SUBTITLE "$(^NameDA) жойылып жатқанша күтіңіз."
  ${LangFileString} MUI_UNTEXT_FINISH_TITLE "Жою аяқталды"
  ${LangFileString} MUI_UNTEXT_FINISH_SUBTITLE "Жою сәтті аяқталды."
  ${LangFileString} MUI_UNTEXT_ABORT_TITLE "Жою тоқтатылды"
  ${LangFileString} MUI_UNTEXT_ABORT_SUBTITLE "Жою сәтті аяқталмады."
!endif

!ifdef MUI_FINISHPAGE
  ${LangFileString} MUI_TEXT_FINISH_INFO_TITLE "$(^NameDA) Орнату аяқталуда"
  ${LangFileString} MUI_TEXT_FINISH_INFO_TEXT "$(^NameDA) компьютеріңізге орнатылды.$\r$\n$\r$\nClick Setup қолданбасын жабу үшін аяқтаңыз."
  ${LangFileString} MUI_TEXT_FINISH_INFO_REBOOT "$(^NameDA). Орнатуды аяқтау үшін компьютерді қайта іске қосу керек. Қазір қайта іске қосқыңыз келе ме?"
!endif

!ifdef MUI_UNFINISHPAGE
  ${LangFileString} MUI_UNTEXT_FINISH_INFO_TITLE "Аяқталуда $(^NameDA) Жою"
  ${LangFileString} MUI_UNTEXT_FINISH_INFO_TEXT "$(^NameDA) компьютеріңізден жойылды.$\r$\n$\r$\nClick Setup қолданбасын жабу үшін аяқтаңыз."
  ${LangFileString} MUI_UNTEXT_FINISH_INFO_REBOOT "$(^NameDA). Жоюды аяқтау үшін компьютерді қайта іске қосу керек. Қазір қайта іске қосқыңыз келе ме?"
!endif

!ifdef MUI_FINISHPAGE | MUI_UNFINISHPAGE
  ${LangFileString} MUI_TEXT_FINISH_REBOOTNOW "Қазір қайта жүктеңіз"
  ${LangFileString} MUI_TEXT_FINISH_REBOOTLATER "Мен кейінірек қолмен қайта жүктегім келеді"
  ${LangFileString} MUI_TEXT_FINISH_RUN "&Іске қосу $(^NameDA)"
  ${LangFileString} MUI_TEXT_FINISH_SHOWREADME "&Менің оқуымды көрсету"
  ${LangFileString} MUI_BUTTONTEXT_FINISH "&Аяқтау"  
!endif

!ifdef MUI_STARTMENUPAGE
  ${LangFileString} MUI_TEXT_STARTMENU_TITLE "Бастау мәзірінің қалтасын таңдаңыз"
  ${LangFileString} MUI_TEXT_STARTMENU_SUBTITLE "$(^NameDA) таңбашалар үшін Бастау мәзірі қалтасын таңдаңыз."
  ${LangFileString} MUI_INNERTEXT_STARTMENU_TOP "Бағдарламаның таңбашаларын жасағыңыз келетін Бастау мәзірі қалтасын таңдаңыз. Жаңа қалта жасау үшін атауды енгізуге де болады."
  ${LangFileString} MUI_INNERTEXT_STARTMENU_CHECKBOX "Таңбашаларды жасамаңыз"
!endif

!ifdef MUI_UNCONFIRMPAGE
  ${LangFileString} MUI_UNTEXT_CONFIRM_TITLE "$(^NameDA) жою"
  ${LangFileString} MUI_UNTEXT_CONFIRM_SUBTITLE "Компьютерден $(^NameDA) алып тастаңыз."
!endif

!ifdef MUI_ABORTWARNING
  ${LangFileString} MUI_TEXT_ABORTWARNING "$(^Name) Setup қолданбасынан шынымен шыққыңыз келе ме?"
!endif

!ifdef MUI_UNABORTWARNING
  ${LangFileString} MUI_UNTEXT_ABORTWARNING "$(^Name) Жоюдан шыққыңыз келетініне сенімдісіз бе?"
!endif

!ifdef MULTIUSER_INSTALLMODEPAGE
  ${LangFileString} MULTIUSER_TEXT_INSTALLMODE_TITLE "Пайдаланушыларды таңдаңыз"
  ${LangFileString} MULTIUSER_TEXT_INSTALLMODE_SUBTITLE "Қай пайдаланушылар үшін орнатқыңыз келетінін таңдаңыз $(^NameDA)."
  ${LangFileString} MULTIUSER_INNERTEXT_INSTALLMODE_TOP "Тек өзіңіз үшін немесе осы компьютердің барлық пайдаланушылары үшін $(^NameDA) орнатқыңыз келетінін таңдаңыз. $(^ClickNext)"
  ${LangFileString} MULTIUSER_INNERTEXT_INSTALLMODE_ALLUSERS "Осы компьютерді пайдаланатын кез келген адам үшін орнатыңыз"
  ${LangFileString} MULTIUSER_INNERTEXT_INSTALLMODE_CURRENTUSER "Мен үшін ғана орнатыңыз"
!endif
