import AppKit
import CoreGraphics
import Foundation
guard CommandLine.arguments.count==3,let pid=Int32(CommandLine.arguments[1]),NSRunningApplication(processIdentifier:pid)?.localizedName=="e28_db_browser" else {fatalError("Expected owned browser")}
let source=CGEventSource(stateID:.privateState)!
guard CommandLine.arguments[2]=="toggle" else {fatalError("Expected owned format toggle")}
let key:CGKeyCode=49
for down in [true,false] {
 let event=CGEvent(keyboardEventSource:source,virtualKey:key,keyDown:down)!
 event.flags=[];event.postToPid(pid)
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))
}
RunLoop.current.run(until:Date(timeIntervalSinceNow:0.7))
print("Owned native browser action completed")
