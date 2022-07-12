import 'dart:convert';

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

        //     Scaffold(
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
      appBar: AppBar(
        leading: IconButton(
            onPressed: () async {
              if (_controller == null) return;
              if (await _controller!.canGoBack()) {
                _controller!.goBack();
              }
            },
            icon: Icon(Icons.arrow_back)),
        title: Text("WebView RustDesk Demo"),
        actions: [
          IconButton(
              onPressed: () async {
                await _controller?.clearCache();
                await _controller?.reload();
              },
              icon: Icon(Icons.refresh))
        ],
      ),
      body: WebView(
        onWebViewCreated: (c) => _controller = c,
        javascriptMode: JavascriptMode.unrestricted,
        javascriptChannels: [
          JavascriptChannel(
              name: 'RustDeskChannel',
              onMessageReceived: (message) {
                debugPrint("onMessageReceived : ${message.message}");
                try {
                  final msg =
                      json.decode(message.message) as Map<String, dynamic>;
                  final id = msg["id"] as String?;
                  final passwordPreset = msg["password"] as String?;
                  if (id == null) return;
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (BuildContext context) => RemotePage(
                          id: id, passwordPreset: passwordPreset ?? ""),
                    ),
                  );
                } catch (e) {
                  debugPrint('onMessageReceived json decode error: $e');
                }
              })
        ].toSet(),
        initialUrl: 'http://192.168.2.29:3030',
        // initialUrl: 'http://114.242.9.52:9099/padb/',
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () async {
          _controller?.runJavascript('RustDeskChannel.onMessage("Test")');
          debugPrint("floatingActionButton tap");
        },
        child: Text("Call"),
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
