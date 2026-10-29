import AppKit
import CoreGraphics
import Foundation
guard CommandLine.arguments.count==3,let pid=Int32(CommandLine.arguments[1]),NSRunningApplication(processIdentifier:pid)?.localizedName=="e28_db_browser" else {fatalError("Expected owned browser")}
let source=CGEventSource(stateID:.privateState)!
func key(_ code:CGKeyCode,_ flags:CGEventFlags=[]) {
 for down in [true,false] {let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:down)!;event.flags=flags;event.postToPid(pid);RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))}
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.6))
}
switch CommandLine.arguments[2] {
 case "open":key(36,.maskCommand)
 case "cancel":key(36)
 case "apply":key(48);key(36)
 default:fatalError("Expected owned format confirmation action")
}
print("Owned native format confirmation action completed")
