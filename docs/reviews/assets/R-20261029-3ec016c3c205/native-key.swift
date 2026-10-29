import AppKit
import CoreGraphics
import Foundation
guard CommandLine.arguments.count==3,let pid=Int32(CommandLine.arguments[1]),NSRunningApplication(processIdentifier:pid)?.localizedName=="e28_db_browser" else {fatalError("Expected owned browser")}
let source=CGEventSource(stateID:.privateState)!
func key(_ code:CGKeyCode,_ flags:CGEventFlags=[]) {
 for down in [true,false] {
  let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:down)!
  event.flags=flags;event.postToPid(pid)
  RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))
 }
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.35))
}
switch CommandLine.arguments[2] {
 case "actual":key(49)
 case "zoom":key(48);key(49)
 case "fit":key(48,.maskShift);key(48,.maskShift);key(49);key(48)
 default:fatalError("Unknown owned image action")
}
RunLoop.current.run(until:Date(timeIntervalSinceNow:0.5))
print("Owned native image action completed")
