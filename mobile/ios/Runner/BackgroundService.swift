import BackgroundTasks
import Darwin
import Foundation

final class BackgroundService {
    private static let taskIdentifier = "com.onyx.sync"
    private typealias BackgroundSyncFunction = @convention(c) (UInt64) -> Int32

    static func register() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: taskIdentifier, using: nil) { task in
            guard let refreshTask = task as? BGAppRefreshTask else {
                task.setTaskCompleted(success: false)
                return
            }
            let success = runNativeSync(taskId: UInt64(Date().timeIntervalSince1970)) == 1
            refreshTask.setTaskCompleted(success: success)
            scheduleNext()
        }
        scheduleNext()
    }

    static func scheduleNext() {
        let request = BGAppRefreshTaskRequest(identifier: taskIdentifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(request)
    }

    private static func runNativeSync(taskId: UInt64) -> Int32 {
        guard let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "mobile_core_background_sync_registered") else {
            return 0
        }
        let function = unsafeBitCast(symbol, to: BackgroundSyncFunction.self)
        return function(taskId)
    }
}
