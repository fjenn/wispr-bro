// SPDX-License-Identifier: GPL-3.0-or-later
//
// Wispr Bro for iPhone: the UIKit scene life cycle for a tao/Tauri app.
//
// An app built with the iOS 27 SDK must use the scene-based life cycle
// (UIKit aborts at launch otherwise: "UIScene life cycle is required for
// apps built with this SDK"). tao (the windowing layer under Tauri) still
// uses the application-delegate life cycle: it makes its UIWindow without a
// scene and listens to the application delegate's callbacks. This file
// bridges the two without touching tao:
//
//  - Info.ios.plist names WBSceneDelegate as the one window scene's delegate;
//  - a window created without a scene (tao's) is given the app's scene;
//  - the scene's life-cycle and open-URL callbacks are passed on to the
//    application delegate's old ones, which tao and Tauri handle, and a URL
//    is also posted as WBOpenURLNotification (object: the NSURL) for the
//    native session code.
//
// Compiled into the Rust library by build.rs; `wb_ios_scene_install` is
// called from `fork::ios::start` before UIKit starts.

#import <UIKit/UIKit.h>
#import <objc/message.h>
#import <objc/runtime.h>

NSNotificationName const WBOpenURLNotification = @"WBOpenURLNotification";

static UIWindowScene *gScene;
static NSHashTable<UIWindow *> *gWindows;

static void WBAdopt(UIWindow *window) {
    if (gScene == nil || window.windowScene != nil) {
        return;
    }
    window.windowScene = gScene;
    if (!window.hidden) {
        [window makeKeyAndVisible];
    }
}

@interface UIWindow (WBScene)
@end

@implementation UIWindow (WBScene)

// Swapped with -initWithFrame: (so the call below runs the original).
- (instancetype)wb_initWithFrame:(CGRect)frame {
    UIWindow *window = [self wb_initWithFrame:frame];
    if (window != nil) {
        [gWindows addObject:window];
        // After the caller finished setting the window up; a window UIKit
        // made for a scene has its scene by then and is left alone.
        dispatch_async(dispatch_get_main_queue(), ^{
            WBAdopt(window);
        });
    }
    return window;
}

@end

@interface WBSceneDelegate : UIResponder <UIWindowSceneDelegate>
@end

@implementation WBSceneDelegate

- (id<UIApplicationDelegate>)appDelegate {
    return UIApplication.sharedApplication.delegate;
}

- (void)openURLContexts:(NSSet<UIOpenURLContext *> *)contexts {
    for (UIOpenURLContext *context in contexts) {
        NSURL *url = context.URL;
        // A session link from the keyboard (wisprbro://…) goes to the Swift
        // session controller, when it is in the app (looked up by name: the
        // Rust library builds without it).
        Class controller = NSClassFromString(@"WBSessionController");
        SEL shared = NSSelectorFromString(@"shared");
        SEL handle = NSSelectorFromString(@"handleOpenWithUrl:");
        if ([controller respondsToSelector:shared]) {
            id session = ((id (*)(id, SEL))objc_msgSend)(controller, shared);
            if ([session respondsToSelector:handle] &&
                ((BOOL (*)(id, SEL, NSURL *))objc_msgSend)(session, handle, url)) {
                continue;
            }
        }
        id<UIApplicationDelegate> delegate = [self appDelegate];
        if ([delegate respondsToSelector:@selector(application:openURL:options:)]) {
            [delegate application:UIApplication.sharedApplication openURL:url options:@{}];
        }
        [NSNotificationCenter.defaultCenter postNotificationName:WBOpenURLNotification object:url];
    }
}

- (void)scene:(UIScene *)scene
    willConnectToSession:(UISceneSession *)session
                 options:(UISceneConnectionOptions *)connectionOptions {
    if (![scene isKindOfClass:UIWindowScene.class]) {
        return;
    }
    gScene = (UIWindowScene *)scene;
    for (UIWindow *window in gWindows.allObjects) {
        WBAdopt(window);
    }
    if (connectionOptions.URLContexts.count > 0) {
        NSSet<UIOpenURLContext *> *contexts = connectionOptions.URLContexts;
        // After launch finished, so every observer is in place.
        dispatch_async(dispatch_get_main_queue(), ^{
            [self openURLContexts:contexts];
        });
    }
}

- (void)sceneDidDisconnect:(UIScene *)scene {
    if (scene == gScene) {
        gScene = nil;
    }
}

- (void)scene:(UIScene *)scene openURLContexts:(NSSet<UIOpenURLContext *> *)URLContexts {
    [self openURLContexts:URLContexts];
}

- (void)sceneDidBecomeActive:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationDidBecomeActive:)]) {
        [delegate applicationDidBecomeActive:UIApplication.sharedApplication];
    }
}

- (void)sceneWillResignActive:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationWillResignActive:)]) {
        [delegate applicationWillResignActive:UIApplication.sharedApplication];
    }
}

- (void)sceneWillEnterForeground:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationWillEnterForeground:)]) {
        [delegate applicationWillEnterForeground:UIApplication.sharedApplication];
    }
}

- (void)sceneDidEnterBackground:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationDidEnterBackground:)]) {
        [delegate applicationDidEnterBackground:UIApplication.sharedApplication];
    }
}

@end

void wb_ios_scene_install(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gWindows = [NSHashTable weakObjectsHashTable];
        Method original = class_getInstanceMethod(UIWindow.class, @selector(initWithFrame:));
        Method ours = class_getInstanceMethod(UIWindow.class, @selector(wb_initWithFrame:));
        if (original != NULL && ours != NULL) {
            method_exchangeImplementations(original, ours);
        }
    });
}
