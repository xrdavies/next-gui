import Foundation
import Vision
import AppKit
var results=[String:[String]]()
for path in CommandLine.arguments.dropFirst() {
 let image=NSImage(contentsOfFile:path)!
 var rect=NSRect(origin:.zero,size:image.size)
 let cg=image.cgImage(forProposedRect:&rect,context:nil,hints:nil)!
 let request=VNRecognizeTextRequest();request.recognitionLevel = .accurate
 try VNImageRequestHandler(cgImage:cg).perform([request])
 results[path]=(request.results ?? []).compactMap{$0.topCandidates(1).first?.string}
}
let data=try JSONSerialization.data(withJSONObject:results,options:[.prettyPrinted,.sortedKeys]);print(String(data:data,encoding:.utf8)!)
