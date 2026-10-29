import AppKit
import CoreGraphics
import Foundation
guard CommandLine.arguments.count==3,let pid=Int32(CommandLine.arguments[1]),NSRunningApplication(processIdentifier:pid)?.localizedName=="e28_db_browser" else {fatalError("Expected owned browser")}
let source=CGEventSource(stateID:.privateState)!
let key:CGKeyCode
switch CommandLine.arguments[2] {case "home":key=115;case "end":key=119;case "toggle":key=49;default:fatalError("Unknown action")}
for down in [true,false] {
 let event=CGEvent(keyboardEventSource:source,virtualKey:key,keyDown:down)!
 event.flags=[];event.postToPid(pid)
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))
}
RunLoop.current.run(until:Date(timeIntervalSinceNow:0.7))
print("Owned native browser action completed")
