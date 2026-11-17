// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sync on iPhone, the UIKit part (see ios.rs): the share sheet for the files
// this phone sends to the Mac.

#import <UIKit/UIKit.h>

// `paths_json`: a JSON array of file paths. Safe from any thread.
void wb_ios_share_files(const char *paths_json) {
    NSData *data = paths_json ? [NSData dataWithBytes:paths_json length:strlen(paths_json)] : nil;
    NSArray *paths = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    if (![paths isKindOfClass:NSArray.class] || paths.count == 0) {
        return;
    }
    NSMutableArray<NSURL *> *items = [NSMutableArray array];
    for (id path in paths) {
        if ([path isKindOfClass:NSString.class]) {
            [items addObject:[NSURL fileURLWithPath:path]];
        }
    }
    dispatch_async(dispatch_get_main_queue(), ^{
        UIWindow *window = nil;
        for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
            if ([scene isKindOfClass:UIWindowScene.class]) {
                window = ((UIWindowScene *)scene).keyWindow ?: ((UIWindowScene *)scene).windows.firstObject;
                if (window != nil) {
                    break;
                }
            }
        }
        UIViewController *top = window.rootViewController;
        while (top.presentedViewController != nil) {
            top = top.presentedViewController;
        }
        if (top == nil) {
            NSLog(@"WBSync: no view controller to present the share sheet from");
            return;
        }
        UIActivityViewController *sheet =
            [[UIActivityViewController alloc] initWithActivityItems:items applicationActivities:nil];
        // iPad: a sheet needs an anchor.
        sheet.popoverPresentationController.sourceView = top.view;
        sheet.popoverPresentationController.sourceRect =
            CGRectMake(CGRectGetMidX(top.view.bounds), CGRectGetMidY(top.view.bounds), 1, 1);
        [top presentViewController:sheet animated:YES completion:nil];
    });
}
