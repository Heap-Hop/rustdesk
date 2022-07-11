import 'package:flutter/material.dart';
import 'package:flutter_hbb/pages/chat_page.dart';
import 'package:flutter_hbb/pages/remote_page.dart';
import 'package:flutter_hbb/pages/server_page.dart';
import 'package:flutter_hbb/pages/settings_page.dart';
import 'package:webview_flutter/webview_flutter.dart';
import '../common.dart';
import '../widgets/overlay.dart';
import 'connection_page.dart';

abstract class PageShape extends Widget {
  final String title = "";
  final Icon icon = Icon(null);
  final List<Widget> appBarActions = [];
}

class HomePage extends StatefulWidget {
  HomePage({Key? key}) : super(key: key);

  @override
  _HomePageState createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  var _selectedIndex = 0;
  final List<PageShape> _pages = [];

  @override
  void initState() {
    super.initState();
    _pages.add(ConnectionPage());
    if (isAndroid) {
      _pages.addAll([chatPage, ServerPage()]);
    }
    _pages.add(SettingsPage());
  }

  @override
  Widget build(BuildContext context) {
    return WillPopScope(
        onWillPop: () async {
          if (_selectedIndex != 0) {
            setState(() {
              _selectedIndex = 0;
            });
          } else {
            return true;
          }
          return false;
        },
        child: WebViewExample()

        // Scaffold(
        //   backgroundColor: MyTheme.grayBg,
        //   appBar: AppBar(
        //     centerTitle: true,
        //     title: Text("RustDesk"),
        //     actions: _pages.elementAt(_selectedIndex).appBarActions,
        //   ),
        //   bottomNavigationBar: BottomNavigationBar(
        //     key: navigationBarKey,
        //     items: _pages
        //         .map((page) =>
        //             BottomNavigationBarItem(icon: page.icon, label: page.title))
        //         .toList(),
        //     currentIndex: _selectedIndex,
        //     type: BottomNavigationBarType.fixed,
        //     selectedItemColor: MyTheme.accent,
        //     unselectedItemColor: MyTheme.darkGray,
        //     onTap: (index) => setState(() {
        //       // close chat overlay when go chat page
        //       if (index == 1 && _selectedIndex != index) {
        //         hideChatIconOverlay();
        //         hideChatWindowOverlay();
        //       }
        //       _selectedIndex = index;
        //     }),
        //   ),
        //   body: _pages.elementAt(_selectedIndex),
        // )
        );
  }
}

class WebViewExample extends StatefulWidget {
  @override
  WebViewExampleState createState() => WebViewExampleState();
}

class WebViewExampleState extends State<WebViewExample> {
  WebViewController? _controller;

  @override
  void initState() {
    super.initState();
    // Enable virtual display.
    if (isAndroid) WebView.platform = AndroidWebView();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text("WebView Open RustDesk Demo")),
      body: WebView(
        onWebViewCreated: (c) => _controller = c,
        javascriptMode: JavascriptMode.unrestricted,
        javascriptChannels: [
          JavascriptChannel(
              name: 'Toast',
              onMessageReceived: (message) {
                debugPrint("onMessageReceived : $message");
                showToast(message.message);
                Navigator.push(
                  context,
                  MaterialPageRoute(
                    builder: (BuildContext context) =>
                        RemotePage(id: "1022661383"),
                  ),
                );
              })
        ].toSet(),
        initialUrl: 'http://114.242.9.52:9099/padb/',
        // initialUrl: 'https://www.bilibili.com/',
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () async {
          if (_controller == null) return;
          if (await _controller!.canGoBack()) {
            _controller!.goBack();
          }
          debugPrint("floatingActionButton tap");
        },
        child: Text("Back"),
      ),
    );
  }
}

class WebHomePage extends StatelessWidget {
  final connectionPage = ConnectionPage();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: MyTheme.grayBg,
      appBar: AppBar(
        centerTitle: true,
        title: Text("RustDesk" + (isWeb ? " (Beta) " : "")),
        actions: connectionPage.appBarActions,
      ),
      body: connectionPage,
    );
  }
}
