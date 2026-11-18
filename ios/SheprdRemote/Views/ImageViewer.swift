import SheprdRemoteCore
import SwiftUI
import UIKit

/// A picture in the chat that opens full screen when tapped, growing out of
/// where it sits and shrinking back when closed. Swiping left and right
/// there goes through the chat's other pictures.
struct ChatPicture: View {
    let picture: GalleryPicture
    let image: UIImage
    @Environment(\.chatPictures) private var pictures
    @State private var showing = false
    @Namespace private var zoom

    var body: some View {
        Button { showing = true } label: {
            Image(uiImage: image).resizable().scaledToFit().frame(maxHeight: 280)
                .clipShape(.rect(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .matchedTransitionSource(id: "picture", in: zoom)
        .accessibilityLabel("Picture")
        .accessibilityHint("Shows it full screen")
        .fullScreenCover(isPresented: $showing) {
            let all = pictures.contains { $0.id == picture.id } ? pictures : [picture]
            ImageViewer(pictures: all, selection: picture.id, first: image)
                .navigationTransition(.zoom(sourceID: "picture", in: zoom))
        }
    }
}

/// Pictures on black, one at a time: swipe left and right between them,
/// pinch or double-tap to zoom, drag to look around, swipe down or tap ✕
/// to close.
struct ImageViewer: View {
    @Environment(\.dismiss) private var dismiss
    let pictures: [GalleryPicture]
    /// Offered for a picture with a tab: closes the viewer, and the caller
    /// opens that tab.
    var openTab: ((GalleryPicture) -> Void)?
    @State private var selection: String
    @State private var chromeHidden = false
    /// The pictures decoded so far, kept only around the one shown.
    @State private var decoded: [String: UIImage]

    /// `first` is the picture at `selection`, already decoded where it was
    /// tapped, so it shows at once as the viewer grows out of it.
    init(pictures: [GalleryPicture], selection: String, first: UIImage? = nil, openTab: ((GalleryPicture) -> Void)? = nil) {
        self.pictures = pictures
        self.openTab = openTab
        _selection = State(initialValue: selection)
        _decoded = State(initialValue: first.map { [selection: $0] } ?? [:])
    }

    var body: some View {
        TabView(selection: $selection) {
            ForEach(pictures) { picture in
                Group {
                    if let image = decoded[picture.id] {
                        ZoomableImage(image: image, tapped: { chromeHidden.toggle() }, closed: { dismiss() })
                    } else {
                        ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
                .tag(picture.id)
                // Again once `forgetFarPictures` let it go and it comes
                // back into view.
                .task(id: decoded[picture.id] == nil) {
                    guard decoded[picture.id] == nil, let image = await picture.image.decoded() else { return }
                    decoded[picture.id] = image
                }
            }
        }
        .tabViewStyle(.page(indexDisplayMode: .never))
        .ignoresSafeArea()
        .background(.black)
        .onChange(of: selection) { forgetFarPictures() }
        .overlay(alignment: .top) {
            if !chromeHidden { chrome.transition(.opacity) }
        }
        .overlay(alignment: .bottom) {
            if !chromeHidden, let openTab, let picture = pictures.first(where: { $0.id == selection }), picture.tab != nil {
                tabButton(picture, open: openTab).transition(.opacity)
            }
        }
        .animation(.easeInOut(duration: 0.2), value: chromeHidden)
        .statusBarHidden(chromeHidden)
        .environment(\.colorScheme, .dark)
    }

    private var chrome: some View {
        HStack(alignment: .top) {
            Button { dismiss() } label: {
                Image(systemName: "xmark").font(.body.bold()).frame(width: 36, height: 36)
            }
            .buttonStyle(.glass).buttonBorderShape(.circle)
            .accessibilityLabel("Close")
            Spacer(minLength: 12)
            position
            Spacer(minLength: 12)
            if let image = decoded[selection] {
                ShareLink(item: Image(uiImage: image), preview: SharePreview("Picture", image: Image(uiImage: image))) {
                    Image(systemName: "square.and.arrow.up").font(.body.bold()).frame(width: 36, height: 36)
                }
                .buttonStyle(.glass).buttonBorderShape(.circle)
                .accessibilityLabel("Share")
            } else {
                Color.clear.frame(width: 36, height: 36)
            }
        }
        .padding(.horizontal, 16)
    }

    /// One tap back to the tab the picture came from, with its agent.
    private func tabButton(_ picture: GalleryPicture, open: @escaping (GalleryPicture) -> Void) -> some View {
        Button {
            open(picture)
            dismiss()
        } label: {
            HStack(spacing: 10) {
                AgentIcon(agent: picture.agent, size: 26)
                Text(picture.caption ?? String(localized: "Open Tab")).font(.body.weight(.semibold)).lineLimit(1)
                Image(systemName: "chevron.forward").font(.footnote.weight(.semibold)).foregroundStyle(.secondary)
            }
            .padding(.leading, 6).padding(.trailing, 14).padding(.vertical, 6)
        }
        .buttonStyle(.glass)
        .buttonBorderShape(.capsule)
        .padding(.horizontal, 24)
        .padding(.bottom, 12)
        .accessibilityLabel("Open \(picture.caption ?? String(localized: "the tab"))")
    }

    /// Which picture this is, and the tab it came from, when there is more
    /// than one to swipe through.
    @ViewBuilder
    private var position: some View {
        let index = pictures.firstIndex { $0.id == selection }
        // The button below names the tab when it is there.
        let caption = index.flatMap { openTab == nil || pictures[$0].tab == nil ? pictures[$0].caption : nil }
        if pictures.count > 1 || caption != nil {
            VStack(spacing: 1) {
                if let index, pictures.count > 1 {
                    Text("\(index + 1) of \(pictures.count)").font(.subheadline.weight(.semibold)).monospacedDigit()
                }
                if let caption {
                    Text(caption).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .padding(.horizontal, 12).padding(.vertical, 6)
            .glassEffect(in: .capsule)
            .accessibilityElement(children: .combine)
        }
    }

    /// Keeps the pictures beside the one shown, so swiping on is instant,
    /// and lets the rest go: a long chat may hold many large pictures.
    private func forgetFarPictures() {
        guard let index = pictures.firstIndex(where: { $0.id == selection }) else { return }
        let near = Set(pictures[max(index - 1, 0)...min(index + 1, pictures.count - 1)].map(\.id))
        decoded = decoded.filter { near.contains($0.key) }
    }
}

/// The picture in a `UIScrollView`, which gives pinch zoom, panning, and
/// bounce as Photos has them. SwiftUI's scroll view does not zoom. Pulling
/// the whole picture down past a threshold closes the viewer; zoomed in, a
/// drag moves around the picture instead.
private struct ZoomableImage: UIViewRepresentable {
    let image: UIImage
    let tapped: () -> Void
    let closed: () -> Void

    func makeUIView(context: Context) -> FittingScrollView {
        let scroll = FittingScrollView()
        scroll.delegate = context.coordinator
        scroll.alwaysBounceVertical = true
        scroll.onResize = { [weak coordinator = context.coordinator] scroll in coordinator?.fit(scroll) }
        scroll.maximumZoomScale = 5
        scroll.showsHorizontalScrollIndicator = false
        scroll.showsVerticalScrollIndicator = false
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.backgroundColor = .clear
        let view = UIImageView(image: image)
        view.contentMode = .scaleAspectFit
        view.isUserInteractionEnabled = true
        scroll.addSubview(view)
        context.coordinator.imageView = view

        let double = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.doubleTapped(_:)))
        double.numberOfTapsRequired = 2
        scroll.addGestureRecognizer(double)
        let single = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.singleTapped))
        single.require(toFail: double)
        scroll.addGestureRecognizer(single)
        return scroll
    }

    func updateUIView(_ scroll: FittingScrollView, context: Context) {
        context.coordinator.parent = self
    }

    func makeCoordinator() -> Coordinator { Coordinator(parent: self) }

    @MainActor
    final class Coordinator: NSObject, UIScrollViewDelegate {
        var parent: ZoomableImage
        weak var imageView: UIImageView?
        /// How far past the top the picture is pulled before it closes.
        private let closeDistance: CGFloat = 110

        init(parent: ZoomableImage) { self.parent = parent }

        /// Fits the whole picture to the view, at the start and whenever
        /// the view's size changes, as on rotation or when a foldable opens.
        func fit(_ scroll: UIScrollView) {
            let size = scroll.bounds.size
            guard let imageView, size.width > 0, size.height > 0 else { return }
            let picture = imageView.image?.size ?? size
            let scale = min(size.width / max(picture.width, 1), size.height / max(picture.height, 1))
            scroll.zoomScale = 1
            imageView.frame = CGRect(origin: .zero, size: CGSize(width: picture.width * scale, height: picture.height * scale))
            scroll.contentSize = imageView.frame.size
            center(scroll)
        }

        func viewForZooming(in scrollView: UIScrollView) -> UIView? { imageView }

        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            center(scrollView)
        }

        func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
            guard scrollView.zoomScale <= 1.01 else { return }
            let pulled = -(scrollView.contentOffset.y + scrollView.contentInset.top)
            if pulled > closeDistance || (pulled > 30 && velocity.y < -1.5) { parent.closed() }
        }

        @objc func singleTapped() { parent.tapped() }

        /// Zooms in on the tapped point, or back out to the whole picture.
        @objc func doubleTapped(_ gesture: UITapGestureRecognizer) {
            guard let scroll = gesture.view as? UIScrollView, let imageView else { return }
            if scroll.zoomScale > 1.01 {
                scroll.setZoomScale(1, animated: true)
            } else {
                let point = gesture.location(in: imageView)
                let scale: CGFloat = 2.5
                let size = CGSize(width: scroll.bounds.width / scale, height: scroll.bounds.height / scale)
                scroll.zoom(to: CGRect(origin: CGPoint(x: point.x - size.width / 2, y: point.y - size.height / 2), size: size), animated: true)
            }
        }

        /// Keeps a picture smaller than the screen in its middle.
        private func center(_ scroll: UIScrollView) {
            guard let imageView else { return }
            let horizontal = max(0, (scroll.bounds.width - imageView.frame.width) / 2)
            let vertical = max(0, (scroll.bounds.height - imageView.frame.height) / 2)
            scroll.contentInset = UIEdgeInsets(top: vertical, left: horizontal, bottom: vertical, right: horizontal)
        }
    }
}

/// A scroll view that reports when its size changes, so the picture can be
/// fit again.
private final class FittingScrollView: UIScrollView {
    var onResize: ((UIScrollView) -> Void)?
    private var laidOutSize: CGSize = .zero

    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.size != laidOutSize else { return }
        laidOutSize = bounds.size
        onResize?(self)
    }
}
